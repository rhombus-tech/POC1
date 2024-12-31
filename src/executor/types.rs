#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub execution_id: u128,
    pub result: Vec<u8>,      // Raw execution result
    pub proof: Vec<u8>,       // Proof from the TEE
    pub enclave_type: EnclaveType,
    pub timestamp: u64,       // From blockchain context
    pub block_height: u64,    // From blockchain context
}

#[derive(Debug, Clone)]
pub struct DualExecutionResult {
    pub execution_id: u128,
    pub sgx_result: ExecutionResult,
    pub sev_result: ExecutionResult,
    pub timestamp: u64,       // From blockchain context
    pub block_height: u64,    // From blockchain context
}
