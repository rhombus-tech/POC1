use wasmlanche::{public, Context};
use crate::{
    types::*,
    state::*,
    core::utils::verify_attestation_report,
};

/// Registers a TEE into the watchdog pool for potential executor replacement
#[public]
pub fn register_ready_tee(
    context: &mut Context,
    enclave_type: EnclaveType,
    keep_id: String,
    attestation_report: Vec<u8>,
    drawbridge_token: Vec<u8>,
) -> Result<()> {
    ensure_initialized(context);
    let caller = context.actor();
    
    // Verify TEE attestation
    verify_attestation_report(context, &attestation_report, &drawbridge_token)?;

    let mut pool = context.get(WatchdogPool())?
        .expect("watchdog pool not initialized");

    // Verify not already in pool
    assert!(
        !pool.ready_tees.iter().any(|(addr, _)| *addr == caller),
        "TEE already in ready pool"
    );

    // Add to ready pool
    pool.ready_tees.push((caller, enclave_type));
    pool.health_status.insert(caller, KeepHealth {
        status: KeepStatus::Healthy,
        memory_usage: MemoryStats::default(),
        last_attestation: context.timestamp(),
        keep_id: keep_id.clone(),
    });

    // Store TEE data
    context.store((
        (WatchdogPool(), pool),
        (KeepId(caller), keep_id),
        (DrawbridgeToken(caller), drawbridge_token),
        (EnclaveType(caller), enclave_type),
    ))?;

    Ok(())
}

/// Replaces a failed executor with a ready TEE from the watchdog pool
#[public]
pub fn replace_executor(
    context: &mut Context,
    failed_executor: Address,
) -> Result<()> {
    ensure_initialized(context);
    
    // Get pools
    let mut executor_pool = context.get(ExecutorPool())?
        .expect("executor pool not initialized");
    let mut watchdog_pool = context.get(WatchdogPool())?
        .expect("watchdog pool not initialized");

    // Get failed executor type
    let failed_type = context.get(EnclaveType(failed_executor))?
        .expect("failed executor type not found");

    // Find compatible replacement
    let replacement_idx = watchdog_pool.ready_tees.iter()
        .position(|(_, e_type)| *e_type == failed_type)
        .ok_or(Error::NoAvailableWatchdog)?;

    // Remove from watchdog pool
    let (replacement_tee, _) = watchdog_pool.ready_tees.remove(replacement_idx);

    // Update executor pool
    match failed_type {
        EnclaveType::IntelSGX => {
            executor_pool.sgx_executor = Some(replacement_tee);
        },
        EnclaveType::AMDSEV => {
            executor_pool.sev_executor = Some(replacement_tee);
        }
    }

    // Update pools and record replacement
    watchdog_pool.last_replacement = context.timestamp();
    
    context.store((
        (ExecutorPool(), executor_pool),
        (WatchdogPool(), watchdog_pool),
    ))?;

    // Emit replacement event
    context.emit_event("ExecutorReplaced", &(failed_executor, replacement_tee))?;

    Ok(())
}

/// Checks health of all TEEs in the watchdog pool
#[public]
pub fn check_watchdog_pool_health(context: &mut Context) -> Result<()> {
    let mut pool = context.get(WatchdogPool())?
        .expect("watchdog pool not initialized");

    // Remove any unhealthy TEEs
    pool.ready_tees.retain(|(addr, _)| {
        if let Some(health) = pool.health_status.get(addr) {
            matches!(health.status, KeepStatus::Healthy)
        } else {
            false
        }
    });

    // Verify minimum pool size
    assert!(
        pool.ready_tees.len() >= pool.min_pool_size,
        "watchdog pool below minimum size"
    );

    context.store(WatchdogPool(), pool)?;
    Ok(())
}

/// Updates health status for a TEE in the watchdog pool
#[public]
pub fn update_tee_health(
    context: &mut Context,
    keep_id: String,
    memory_stats: MemoryStats,
) -> Result<()> {
    let mut pool = context.get(WatchdogPool())?
        .expect("watchdog pool not initialized");

    let caller = context.actor();
    
    if let Some(health) = pool.health_status.get_mut(&caller) {
        health.memory_usage = memory_stats;
        health.last_attestation = context.timestamp();
    }

    context.store(WatchdogPool(), pool)?;
    Ok(())
}
