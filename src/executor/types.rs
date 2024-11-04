#[derive(Debug)]
pub struct ExecutorRequest {
    pub context: Context,
    pub execution_id: u128,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub struct ExecutionResult {
    pub execution_id: u128,
    pub result_hash: Vec<u8>,
    pub keep_id: String,
    pub attestation: AttestationResult,
    pub drawbridge_token: DrawbridgeToken,
    pub timestamp: u64,
}

#[derive(Debug)]
pub struct KeepExecutionResult {
    pub hash: Vec<u8>,
    pub height: u64,
    pub state_update: Option<Vec<u8>>,
}
