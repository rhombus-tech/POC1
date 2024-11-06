use prometheus::{Counter, Histogram, register_counter, register_histogram};

pub struct PoolMetrics {
    pub execution_time: Histogram,
    pub successful_executions: Counter,
    pub failed_executions: Counter,
    pub successful_challenges: Counter,
    pub failed_challenges: Counter,
    pub executor_replacements: Counter,
}

impl PoolMetrics {
    pub fn new() -> Self {
        Self {
            execution_time: register_histogram!(
                "executor_execution_time_seconds",
                "Time spent executing requests"
            ).unwrap(),
            // ... other metrics
        }
    }
}
