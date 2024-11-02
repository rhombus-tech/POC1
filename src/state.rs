use wasmlanche::{state_schema, Address};
use crate::types::*;

state_schema! {
    /// System state
    CurrentPhase() => Phase,
    SystemInitialized() => bool,
    LastGlobalUpdate() => u64,

    /// Pools
    ExecutorPool() => ExecutorPool,
    WatchdogPool() => WatchdogPool,

    /// Operator and enclave data
    EnclaveType(Address) => EnclaveType,
    OperatorData(String) => Operator,
    AttestationStatus(Address) => bool,
    HeartbeatTimestamp(Address) => u64,

    /// Contract management
    Contract(u128) => Contract,
    ContractCount() => u128,
    ActiveContracts() => Vec<u128>,

    /// Challenge system
    Challenge(u128) => Challenge,
    ActiveChallenges() => Vec<u128>,
    ChallengeCount() => u128,

    /// Verification and security
    OperatorHash() => Vec<u8>,
    StateRoot() => Vec<u8>,
    VerificationProof(u128) => Vec<u8>,

    /// External contract references
    TokenContract() => Address,
    GovernanceContract() => Address,

     /// Enarx Keep identifiers
    KeepId(Address) => String,
    /// Drawbridge attestation tokens
    DrawbridgeToken(Address) => Vec<u8>,

    /// Stores execution results for verification
    ExecutionResult(u128) => ExecutionResult,
    /// Maps execution IDs to verification status
    ExecutionVerified(u128) => bool,
    /// Tracks pending verifications
    PendingVerifications() => Vec<u128>,
    /// Stores mismatched executions for analysis
    ExecutionMismatches(u128) => (ExecutionResult, ExecutionResult),
}

// Helper functions for state management
pub fn ensure_initialized(context: &mut wasmlanche::Context) {
    assert!(
        context.get(SystemInitialized()).expect("state corrupt").unwrap_or(false),
        "system not initialized"
    );
}

pub fn ensure_phase(context: &mut wasmlanche::Context, expected_phase: Phase) {
    let current_phase = context
        .get(CurrentPhase())
        .expect("state corrupt")
        .unwrap_or(Phase::None);
    assert!(
        current_phase == expected_phase,
        "invalid phase: expected {:?}, got {:?}",
        expected_phase,
        current_phase
    );
}

pub fn update_global_state(context: &mut wasmlanche::Context) {
    context
        .store_by_key(LastGlobalUpdate(), context.timestamp())
        .expect("failed to update global state");
}
