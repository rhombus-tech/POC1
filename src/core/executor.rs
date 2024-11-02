use wasmlanche::{public, Context, Address};
use crate::{
    types::*,
    state::*,
    core::utils::verify_attestation_report,
};

#[public]
pub fn register_executor(
    context: &mut Context,
    enclave_type: EnclaveType,
    keep_id: String,
    attestation_report: Vec<u8>,
    drawbridge_token: Vec<u8>,
) {
    ensure_initialized(context);
    ensure_phase(context, Phase::Creation);

    let caller = context.actor();
    
    // Verify Enarx Keep attestation
    assert!(
        verify_attestation_report(
            context,
            &attestation_report,
            &drawbridge_token,
            enclave_type
        ),
        "invalid attestation"
    );

    let mut executor_pool = context
        .get(ExecutorPool())
        .expect("state corrupt")
        .expect("executor pool not initialized");

    match enclave_type {
        EnclaveType::IntelSGX => {
            assert!(executor_pool.sgx_executor.is_none(), "SGX executor slot already filled");
            executor_pool.sgx_executor = Some(caller);
        },
        EnclaveType::AMDSEV => {
            assert!(executor_pool.sev_executor.is_none(), "SEV executor slot already filled");
            executor_pool.sev_executor = Some(caller);
        }
    }

    // Store updated state with Enarx info
    context
        .store((
            (ExecutorPool(), executor_pool.clone()),
            (EnclaveType(caller), enclave_type),
            (KeepId(caller), keep_id),              // New
            (DrawbridgeToken(caller), drawbridge_token), // New
            (AttestationStatus(caller), true),
            (HeartbeatTimestamp(caller), context.timestamp()),
        ))
        .expect("failed to register executor");

    if executor_pool.sgx_executor.is_some() && executor_pool.sev_executor.is_some() {
        transition_to_executing(context);
    }
}
    ensure_initialized(context);
    ensure_phase(context, Phase::Creation);

    let caller = context.actor();
    
    // Verify operator exists and is initialized
    let mut operator = context
        .get(OperatorData(operator_address.clone()))
        .expect("state corrupt")
        .expect("operator not found");

    assert!(operator.initialized, "operator not initialized");

    // Verify attestation
    verify_attestation_report(context, &attestation_report, &tee_signature);

    // Update operator data
    operator.attestation_report = attestation_report;
    operator.last_heartbeat = context.timestamp();

    let mut executor_pool = context
        .get(ExecutorPool())
        .expect("state corrupt")
        .expect("executor pool not initialized");

    // Register based on enclave type
    match enclave_type {
        EnclaveType::IntelSGX => {
            assert!(executor_pool.sgx_executor.is_none(), "SGX executor slot already filled");
            executor_pool.sgx_executor = Some(caller);
        },
        EnclaveType::AMDSEV => {
            assert!(executor_pool.sev_executor.is_none(), "SEV executor slot already filled");
            executor_pool.sev_executor = Some(caller);
        }
    }

    // Store updated state
    context
        .store((
            (ExecutorPool(), executor_pool.clone()),
            (EnclaveType(caller), enclave_type),
            (OperatorData(operator_address), operator),
            (AttestationStatus(caller), true),
            (HeartbeatTimestamp(caller), context.timestamp()),
        ))
        .expect("failed to register executor");

    // Check if we can transition to executing phase
    if executor_pool.sgx_executor.is_some() && executor_pool.sev_executor.is_some() {
        transition_to_executing(context);
    }
}

#[public]
pub fn submit_heartbeat(context: &mut Context) {
    ensure_initialized(context);
    let caller = context.actor();
    let timestamp = context.timestamp();

    // Verify caller is either executor or watchdog
    let executor_pool = context
        .get(ExecutorPool())
        .expect("state corrupt")
        .expect("executor pool not initialized");

    let watchdog_pool = context
        .get(WatchdogPool())
        .expect("state corrupt")
        .expect("watchdog pool not initialized");

    let is_executor = executor_pool.sgx_executor == Some(caller) || 
                     executor_pool.sev_executor == Some(caller);
    let is_watchdog = watchdog_pool.watchdogs.iter().any(|(addr, _)| *addr == caller);

    assert!(is_executor || is_watchdog, "unauthorized caller");

    // Update heartbeat timestamp
    context
        .store_by_key(HeartbeatTimestamp(caller), timestamp)
        .expect("failed to update heartbeat");

    // If executor, update execution count
    if is_executor {
        let mut pool = executor_pool;
        pool.last_execution_time = timestamp;
        pool.execution_count += 1;
        context
            .store_by_key(ExecutorPool(), pool)
            .expect("failed to update executor pool");
    }
}

fn transition_to_executing(context: &mut Context) {
    context
        .store_by_key(CurrentPhase(), Phase::Executing)
        .expect("failed to transition to executing");
    
    update_global_state(context);
}
