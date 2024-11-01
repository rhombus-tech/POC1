use wasmlanche::{public, Context, ExternalCallContext};
use crate::{
    types::*,
    state::*,
    core::utils::call_args_from_address,
};

pub fn get_governance_context(context: &mut Context) -> ExternalCallContext {
    let governance_address = context
        .get(GovernanceContract())
        .expect("state corrupt")
        .expect("governance contract not initialized");
    
    context.to_extern(call_args_from_address(governance_address))
}

#[public]
pub fn create_governance_proposal(
    context: &mut Context,
    proposal_type: Vec<u8>,
    proposal_ Vec<u8>,
) {
    ensure_initialized(context);
    let caller = context.actor();

    // Verify caller is executor or watchdog
    let executor_pool = context
        .get(ExecutorPool())
        .expect("state corrupt")
        .expect("executor pool not initialized");
    
    let watchdog_pool = context
        .get(WatchdogPool())
        .expect("state corrupt")
        .expect("watchdog pool not initialized");

    let is_participant = executor_pool.sgx_executor == Some(caller) || 
                        executor_pool.sev_executor == Some(caller) ||
                        watchdog_pool.watchdogs.iter().any(|(addr, _)| *addr == caller);

    assert!(is_participant, "unauthorized proposer");

    // Forward to governance contract
    let governance_context = get_governance_context(context);
    let result = context.call(
        governance_context,
        "create_proposal",
        &[proposal_type, proposal_data],
    );

    assert!(result.is_ok(), "governance proposal creation failed");
}

#[public]
pub fn execute_governance_decision(
    context: &mut Context,
    proposal_id: u128,
    execution_ Vec<u8>,
) {
    ensure_initialized(context);
    
    // Verify caller is governance contract
    let governance_address = context
        .get(GovernanceContract())
        .expect("state corrupt")
        .expect("governance contract not initialized");

    assert!(context.actor() == governance_address, "unauthorized executor");

    // Execute decision based on proposal type
    execute_governance_action(context, proposal_id, &execution_data);
}

fn execute_governance_action(
    context: &mut Context,
    proposal_id: u128,
    execution_ &[u8],
) {
    update_global_state(context);
}
