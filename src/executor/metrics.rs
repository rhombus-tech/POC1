use prometheus::{Counter, Histogram};

pub struct ExecutorMetrics {
    pub execution_time: Histogram,
    pub successful_executions: Counter,
    pub failed_executions: Counter,
    pub attestation_renewals: Counter,
    pub token_refreshes: Counter,
}

impl ExecutorMetrics {
    pub fn new() -> Self {
        // Initialize metrics
        unimplemented!()
    }
}

// Error handling
#[derive(Debug)]
pub enum Error {
    KeepError(enarx::Error),
    InvalidAttestation,
    ExecutionFailed(String),
    StateUpdateFailed(String),
}
