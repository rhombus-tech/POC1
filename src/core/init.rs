use wasmlanche::{public, Context, Address};
use crate::{
    types::*,
    state::*,
};

#[public]
pub fn init(
    context: &mut Context,
    sgx_operator: String,
    sev_operator: String,
    token_contract: Address,
    governance_contract: Address,
) {
    // Ensure system isn't already initialized
    assert!(
        !context.get(SystemInitialized()).expect("state corrupt").unwrap_or(false),
        "system already initialized"
    );

    // Initialize phase
    context
        .store_by_key(CurrentPhase(), Phase::Creation)
        .expect("failed to set initial phase");

    // Initialize empty pools
    let executor_pool = ExecutorPool {
        sgx_executor: None,
        sev_executor: None,
        last_execution_time: context.timestamp(),
        execution_count: 0,
        failed_attempts: 0,
    };

    let watchdog_pool = WatchdogPool {
        watchdogs: Vec::new(),
        active_challenges: Vec::new(),
        last_verification: context.timestamp(),
    };

    // Initialize operators
    let sgx_op = Operator {
        initialized: true,
        tee_signature_address: sgx_operator.clone(),
        tee_encryption_key: Vec::new(),
        attestation_report: Vec::new(),
        last_heartbeat: context.timestamp(),
        challenges_initiated: 0,
        challenges_responded: 0,
    };

    let sev_op = Operator {
        initialized: true,
        tee_signature_address: sev_operator.clone(),
        tee_encryption_key: Vec::new(),
        attestation_report: Vec::new(),
        last_heartbeat: context.timestamp(),
        challenges_initiated: 0,
        challenges_responded: 0,
    };

    // Store initial state
    context
        .store((
            (SystemInitialized(), true),
            (ExecutorPool(), executor_pool),
            (WatchdogPool(), watchdog_pool),
            (OperatorData(sgx_operator), sgx_op),
            (OperatorData(sev_operator), sev_op),
            (TokenContract(), token_contract),
            (GovernanceContract(), governance_contract),
            (LastGlobalUpdate(), context.timestamp()),
        ))
        .expect("failed to initialize system state");

    // Initialize contract tracking
    context
        .store((
            (ContractCount(), 0),
            (ChallengeCount(), 0),
            (ActiveContracts(), Vec::new()),
            (ActiveChallenges(), Vec::new()),
        ))
        .expect("failed to initialize tracking state");
}
