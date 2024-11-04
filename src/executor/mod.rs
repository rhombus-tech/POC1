use wasmlanche::{Context, Address};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use thread_local::ThreadLocal;

/// Request to be processed by executors
#[derive(Debug)]
struct ExecutorRequest {
    execution_id: u128,
    payload: Vec<u8>,
    response_sender: Option<oneshot::Sender<Result<ExecutionResult, Error>>>,
}

/// Executor worker that handles request processing
struct ExecutorWorker {
    /// Enclave type (SGX or SEV)
    enclave_type: EnclaveType,
    /// Cached state
    cached_state: Option<Vec<u8>>,
    /// Last verified block height
    last_height: u64,
    /// Metrics
    metrics: ExecutorMetrics,
}

impl ExecutorWorker {
    fn new(enclave_type: EnclaveType) -> Self {
        Self {
            enclave_type,
            cached_state: None,
            last_height: 0,
            metrics: ExecutorMetrics::new(),
        }
    }

    fn process_request_batch(&mut self, mut requests: Vec<ExecutorRequest>) {
        // Get latest state
        self.update_cached_state();

        // Process batch
        let results = requests.iter_mut().map(|request| {
            let timer = self.metrics.execution_time.start_timer();
            let result = self.execute_in_enclave(&request.payload);
            timer.observe_duration();
            result
        }).collect::<Vec<_>>();

        // Submit results
        for (request, result) in requests.iter_mut().zip(results) {
            if let Some(sender) = request.response_sender.take() {
                sender.send(result).unwrap();
            }
        }

        // Update state if needed
        self.store_new_state();
    }

    fn work(&mut self, receiver: Receiver<ExecutorRequest>) {
        while let Ok(request) = receiver.recv() {
            let mut batch = vec![request];
            
            // Batch requests
            while batch.len() < MAX_BATCH_SIZE {
                match receiver.try_recv() {
                    Ok(req) => batch.push(req),
                    Err(_) => break
                }
            }

            self.process_request_batch(batch);
        }
    }
}

/// Main executor service
pub struct ExecutorService {
    /// Channel for submitting requests
    request_sender: Mutex<Sender<ExecutorRequest>>,
    /// Thread-local sender
    tl_sender: ThreadLocal<Sender<ExecutorRequest>>,
    /// Metrics
    metrics: ServiceMetrics,
}

impl ExecutorService {
    pub fn new(enclave_type: EnclaveType) -> Self {
        let (sender, receiver) = channel();
        
        // Spawn worker thread
        std::thread::spawn(move || {
            ExecutorWorker::new(enclave_type).work(receiver);
        });

        Self {
            request_sender: Mutex::new(sender),
            tl_sender: ThreadLocal::new(),
            metrics: ServiceMetrics::new(),
        }
    }

    pub fn submit_execution(
        &self,
        context: &mut Context, 
        execution_id: u128,
        payload: Vec<u8>,
    ) -> impl Future<Item = ExecutionResult, Error = Error> {
        let (response_sender, response_receiver) = oneshot::channel();

        let request = ExecutorRequest {
            execution_id,
            payload,
            response_sender: Some(response_sender),
        };

        self.get_sender().send(request).unwrap();

        response_receiver.map_err(|e| Error::from(e))
    }

    fn get_sender(&self) -> &Sender<ExecutorRequest> {
        self.tl_sender.get_or(|| {
            let sender = self.request_sender.lock().unwrap();
            Box::new(sender.clone())
        })
    }
}
