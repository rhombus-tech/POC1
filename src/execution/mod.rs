use wasmlanche::{public, Context, Address};
use crate::{
    types::*,
    state::*,
    challenge::*,  // For creating challenges
};

#[public]
pub fn submit_execution_result(
    context: &mut Context,
    execution_id: u128,
    result_hash: Vec<u8>,
) {
    let caller = context.actor();
    
    // Verify caller is an executor
    let executor_pool = context
        .get(ExecutorPool())
        .expect("state corrupt")
        .expect("executor pool not initialized");

    let enclave_type = if Some(caller) == executor_pool.sgx_executor {
        EnclaveType::IntelSGX
    } else if Some(caller) == executor_pool.sev_executor {
        EnclaveType::AMDSEV
    } else {
        panic!("unauthorized executor");
    };

    let result = ExecutionResult {
        result_hash,
        execution_id,
        executor: caller,
        enclave_type,
        timestamp: context.timestamp(),
        block_height: context.block_height(),
    };

    // Store result
    context
        .store_by_key(ExecutionResult(execution_id), result.clone())
        .expect("failed to store result");

    // Add to pending verifications if this is the first result
    let mut pending = context
        .get(PendingVerifications())
        .expect("state corrupt")
        .unwrap_or_default();

    if !pending.contains(&execution_id) {
        pending.push(execution_id);
        context
            .store_by_key(PendingVerifications(), pending)
            .expect("failed to update pending verifications");
    } else {
        // If this is the second result, verify match
        verify_execution_match(context, execution_id);
    }
}

fn verify_execution_match(context: &mut Context, execution_id: u128) {
    let result = context
        .get(ExecutionResult(execution_id))
        .expect("state corrupt")
        .expect("no execution result found");

    // Get both executor results
    let sgx_result = get_executor_result(context, execution_id, EnclaveType::IntelSGX);
    let sev_result = get_executor_result(context, execution_id, EnclaveType::AMDSEV);

    match (sgx_result, sev_result) {
        (Some(sgx), Some(sev)) => {
            if sgx.result_hash == sev.result_hash {
                // Results match
                context
                    .store_by_key(ExecutionVerified(execution_id), true)
                    .expect("failed to mark verification");
                
                // Log successful verification
                log_verification_success(context, execution_id, &sgx, &sev);
            } else {
                // Results don't match - store mismatch and trigger challenge
                context
                    .store_by_key(ExecutionMismatches(execution_id), (sgx.clone(), sev.clone()))
                    .expect("failed to store mismatch");
                
                handle_execution_mismatch(context, execution_id);
                
                // Log mismatch
                log_verification_failure(context, execution_id, &sgx, &sev);
            }
        },
        _ => {
            // Still waiting for both results
            return;
        }
    }

    // Remove from pending verifications
    let mut pending = context
        .get(PendingVerifications())
        .expect("state corrupt")
        .unwrap_or_default();
    pending.retain(|&id| id != execution_id);
    context
        .store_by_key(PendingVerifications(), pending)
        .expect("failed to update pending verifications");
}

fn handle_execution_mismatch(context: &mut Context, execution_id: u128) {
    // Transition to challenge phase
    context
        .store_by_key(CurrentPhase(), Phase::ChallengeExecutor)
        .expect("failed to update phase");

    // Create challenges for both executors to provide proof of their results
    let (sgx, sev) = context
        .get(ExecutionMismatches(execution_id))
        .expect("state corrupt")
        .expect("no mismatch found");

    // Create challenge for verification
    let challenge_data = create_verification_challenge(execution_id, &sgx, &sev);

    // Store challenge for both executors
    create_dual_challenge(context, sgx.executor, sev.executor, challenge_data);
}

#[public]
pub fn verify_execution(
    context: &mut Context,
    execution_id: u128,
) -> bool {
    context
        .get(ExecutionVerified(execution_id))
        .expect("state corrupt")
        .unwrap_or(false)
}

#[public]
pub fn get_execution_result(
    context: &mut Context,
    execution_id: u128,
) -> Option<ExecutionResult> {
    context
        .get(ExecutionResult(execution_id))
        .expect("state corrupt")
}

#[public]
pub fn get_pending_verifications(
    context: &mut Context,
) -> Vec<u128> {
    context
        .get(PendingVerifications())
        .expect("state corrupt")
        .unwrap_or_default()
}

#[public]
pub fn get_verification_mismatch(
    context: &mut Context,
    execution_id: u128,
) -> Option<(ExecutionResult, ExecutionResult)> {
    context
        .get(ExecutionMismatches(execution_id))
        .expect("state corrupt")
}

// Helper functions
fn get_executor_result(
    context: &mut Context,
    execution_id: u128,
    enclave_type: EnclaveType,
) -> Option<ExecutionResult> {
    if let Some(result) = context.get(ExecutionResult(execution_id)).expect("state corrupt") {
        if result.enclave_type == enclave_type {
            return Some(result);
        }
    }
    None
}

fn create_verification_challenge(
    execution_id: u128,
    sgx_result: &ExecutionResult,
    sev_result: &ExecutionResult,
) -> Vec<u8> {
    // Create challenge data including:
    // - Execution ID
    // - Both result hashes
    // - Block height and timestamp
    let mut challenge_data = Vec::new();
    challenge_data.extend(&execution_id.to_le_bytes());
    challenge_data.extend(&sgx_result.result_hash);
    challenge_data.extend(&sev_result.result_hash);
    challenge_data
}

fn create_dual_challenge(
    context: &mut Context,
    sgx_executor: Address,
    sev_executor: Address,
    challenge_ Vec<u8>,
) {
    // Create challenge for SGX executor
    challenge_executor(
        context,
        sgx_executor,
        ChallengeType::ExecutionVerification,
        challenge_data.clone(),
    );

    // Create challenge for SEV executor
    challenge_executor(
        context,
        sev_executor,
        ChallengeType::ExecutionVerification,
        challenge_data,
    );
}

fn log_verification_success(
    context: &mut Context,
    execution_id: u128,
    sgx_result: &ExecutionResult,
    sev_result: &ExecutionResult,
) {
    wasmlanche::dbg!(
        "Execution verification successful",
        execution_id,
        sgx_result.block_height,
        sev_result.block_height,
    );
}

fn log_verification_failure(
    context: &mut Context,
    execution_id: u128,
    sgx_result: &ExecutionResult,
    sev_result: &ExecutionResult,
) {
    wasmlanche::dbg!(
        "Execution verification failed",
        execution_id,
        sgx_result.result_hash,
        sev_result.result_hash,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::common::*;

    #[test]
    fn test_matching_execution_results() {
        let mut context = setup();
        let (sgx_executor, sev_executor, _) = setup_system(&mut context);

        let execution_id = 1u128;
        let result_hash = vec![1u8; 32];

        // Submit SGX result
        context.set_caller(sgx_executor);
        submit_execution_result(&mut context, execution_id, result_hash.clone());

        // Submit matching SEV result
        context.set_caller(sev_executor);
        submit_execution_result(&mut context, execution_id, result_hash.clone());

        // Verify results matched
        assert!(verify_execution(&mut context, execution_id));
        
        // Verify no pending verifications
        let pending = get_pending_verifications(&mut context);
        assert!(pending.is_empty());
    }

    #[test]
    fn test_mismatched_execution_results() {
        let mut context = setup();
        let (sgx_executor, sev_executor, _) = setup_system(&mut context);

        let execution_id = 1u128;
        
        // Submit different results
        context.set_caller(sgx_executor);
        submit_execution_result(&mut context, execution_id, vec![1u8; 32]);

        context.set_caller(sev_executor);
        submit_execution_result(&mut context, execution_id, vec![2u8; 32]);

        // Verify mismatch was detected
        assert!(!verify_execution(&mut context, execution_id));
        assert_eq!(get_current_phase(&mut context), Phase::ChallengeExecutor);

        // Verify mismatch was stored
        let (sgx, sev) = get_verification_mismatch(&mut context, execution_id).unwrap();
        assert_ne!(sgx.result_hash, sev.result_hash);
    }

    #[test]
    #[should_panic(expected = "unauthorized executor")]
    fn test_unauthorized_result_submission() {
        let mut context = setup();
        let unauthorized = Address::from([99u8; 32]);

        context.set_caller(unauthorized);
        submit_execution_result(&mut context, 1u128, vec![0u8; 32]);
    }

    #[test]
    fn test_partial_verification() {
        let mut context = setup();
        let (sgx_executor, _, _) = setup_system(&mut context);

        let execution_id = 1u128;

        // Submit only SGX result
        context.set_caller(sgx_executor);
        submit_execution_result(&mut context, execution_id, vec![1u8; 32]);

        // Verify still pending
        let pending = get_pending_verifications(&mut context);
        assert!(pending.contains(&execution_id));
        assert!(!verify_execution(&mut context, execution_id));
    }
}
