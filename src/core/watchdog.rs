use wasmlanche::{public, Context};
use crate::{
    types::*,
    state::*,
    core::utils::verify_attestation_report,
};

#[public]
pub fn register_watchdog(
    context: &mut Context,
    enclave_type: EnclaveType,
    attestation_report: Vec<u8>,
    tee_signature: Vec<u8>,
) {
    ensure_initialized(context);
    let phase = get_current_phase(context);
    assert!(
        phase == Phase::Creation || phase == Phase::Executing,
        "invalid phase for watchdog registration"
    );

    let caller = context.actor();
    
    // Verify attestation
    verify_attestation_report(context, &attestation_report, &tee_signature);

    let mut watchdog_pool = context
        .get(WatchdogPool())
        .expect("state corrupt")
        .expect("watchdog pool not initialized");

    // Verify not already registered
    assert!(
        !watchdog_pool.watchdogs.iter().any(|(addr, _)| *addr == caller),
        "watchdog already registered"
    );

    // Add to pool
    watchdog_pool.watchdogs.push((caller, enclave_type));

    // Store updated state
    context
        .store((
            (WatchdogPool(), watchdog_pool),
            (EnclaveType(caller), enclave_type),
            (AttestationStatus(caller), true),
            (HeartbeatTimestamp(caller), context.timestamp()),
        ))
        .expect("failed to register watchdog");
}

#[public]
pub fn get_current_phase(context: &mut Context) -> Phase {
    ensure_initialized(context);
    context
        .get(CurrentPhase())
        .expect("state corrupt")
        .unwrap_or(Phase::None)
}

#[public]
pub fn get_system_stats(context: &mut Context) -> (u128, u128, u64) {
    ensure_initialized(context);
    (
        context.get(ContractCount()).expect("state corrupt").unwrap_or(0),
        context.get(ChallengeCount()).expect("state corrupt").unwrap_or(0),
        context.get(LastGlobalUpdate()).expect("state corrupt").unwrap_or(0),
    )
}
