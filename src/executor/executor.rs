use super::Executor;
use crate::enarx::EnarxConfig;

pub struct ExecutorWorker {
    executor: Executor,
    metrics: ExecutorMetrics,
}

impl ExecutorWorker {
    pub fn new(config: EnarxConfig, enclave_type: EnclaveType) -> Result<Self, Error> {
        Ok(Self {
            executor: Executor::new(config, enclave_type)?,
            metrics: ExecutorMetrics::new(),
        })
    }

    pub fn process_request_batch(&mut self, requests: Vec<ExecutorRequest>) -> Vec<ExecutionResult> {
        let mut results = Vec::new();
        
        for request in requests {
            let timer = self.metrics.execution_time.start_timer();
            
            match self.executor.execute(
                &mut request.context,
                request.execution_id,
                request.payload,
            ) {
                Ok(result) => {
                    self.metrics.successful_executions.inc();
                    results.push(result);
                },
                Err(e) => {
                    self.metrics.failed_executions.inc();
                    // Handle error based on type
                    match e {
                        Error::InvalidAttestation => {
                            // Trigger replacement if attestation fails
                            self.request_replacement();
                        },
                        _ => {
                            // Log other errors
                            error!("Execution failed: {:?}", e);
                        }
                    }
                }
            }
            
            timer.observe_duration();
        }

        results
    }

    fn request_replacement(&self) {
        // Notify watchdogs that this executor needs replacement
        // This would trigger the challenge/replacement process
    }
}
