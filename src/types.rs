use wasmlanche::Address;

#[derive(Debug, Clone, PartialEq)]
pub enum EnclaveType {
    IntelSGX,
    AMDSEV,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    None,
    Creation,
    Executing,
    ChallengeExecutor,
    ChallengeWatchdog,
    Crashed,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChallengeType {
    Attestation,
    Execution,
    StateVerification,
    HeartbeatMissed,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChallengeStatus {
    Pending,
    Responded,
    Verified,
    Failed,
    Expired,
}

#[derive(Debug, Clone)]
pub struct Operator {
    initialized: bool,
    keep_id: String,           // New: Enarx Keep identifier
    attestation_report: Vec<u8>,
    drawbridge_token: Vec<u8>, // New: Enarx attestation token
    last_heartbeat: u64,
    challenges_initiated: u64,
    challenges_responded: u64,
}

#[derive(Debug, Clone)]
pub struct ExecutorPool {
    pub sgx_executor: Option<Address>,
    pub sev_executor: Option<Address>,
    pub last_execution_time: u64,
    pub execution_count: u64,
    pub failed_attempts: u64,
}

#[derive(Debug, Clone)]
pub struct WatchdogPool {
    pub watchdogs: Vec<(Address, EnclaveType)>,
    pub active_challenges: Vec<Challenge>,
    pub last_verification: u64,
}

#[derive(Debug, Clone)]
pub struct Challenge {
    pub id: u128,
    pub challenger: Address,
    pub challenged: Address,
    pub challenge_type: ChallengeType,
    pub challenge_ Vec<u8>,
    pub response_deadline: u64,
    pub status: ChallengeStatus,
    pub verification_proofs: Vec<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct ChallengeProof {
    pub challenge_id: u128,
    pub proof_ Vec<u8>,
    pub timestamp: u64,
    pub witness_signatures: Vec<(Address, Vec<u8>)>,
}

#[derive(Debug, Clone)]
pub struct TokenInteraction {
    pub token_address: Address,
    pub amount: u64,
    pub interaction_type: TokenInteractionType,
}

#[derive(Debug, Clone)]
pub enum TokenInteractionType {
    Stake,
    Unstake,
    Reward,
}

#[derive(Debug, Clone)]
pub struct Contract {
    pub id: u128,
    pub phase: Phase,
    pub creation_time: u64,
    pub incremental_tx_hash: Vec<u8>,
    pub executor_pool: ExecutorPool,
    pub watchdog_pool: WatchdogPool,
    pub creation_operator: String,
    pub code_hash: [u8; 32],
    pub exec_challenge_hash: Vec<u8>,
    pub watchdog_challenge_hash: Vec<u8>,
    pub deadline: u64,
    pub state_root: Vec<u8>,
    pub last_verified_block: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionResult {
    pub result_hash: Vec<u8>,      // Checksum of execution result
    pub execution_id: u128,        // Unique ID for this execution
    pub executor: Address,         // Address of executor
    pub enclave_type: EnclaveType,
    pub timestamp: u64,
    pub block_height: u64,
}
