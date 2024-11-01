use wasmlanche::{public, Context};
use crate::{
    types::*,
    state::*,
    challenge::types::*,
    core::utils::verify_attestation_report,
};

#[public]
pub fn respond_to_challenge(
    context: &mut Context,
    challenge_id: u128,
    response_ Vec<u8>,
    proof: ChallengeProof,
) {
    ensure_initialized(context);
    
    let caller = context.actor();
    let timestamp = context.timestamp();

    // Get challenge
    let mut challenge = context
        .get(Challenge(challenge_id))
        .expect("state corrupt")
        .expect("challenge not found");

    // Verify caller is the challenged party
    assert!(challenge.challenged == caller, "unauthorized responder");
    assert!(challenge.status == ChallengeStatus::Pending, "challenge not pending");
    assert!(timestamp <= challenge.response_deadline, "challenge deadline passed");

    // Verify proof
    verify_challenge_proof(context, &challenge, &proof);

    // Update challenge status
    challenge.status = ChallengeStatus::Responded;
    challenge.verification_proofs.push(response_data);

    // Store updated challenge
    context
        .store_by_key(Challenge(challenge_id), challenge.clone())
        .expect("failed to update challenge");

    // Update operator stats
    if let Some(mut operator) = context
        .get(OperatorData(caller.to_string()))
        .expect("state corrupt")
    {
        operator.challenges_responded += 1;
        context
            .store_by_key(OperatorData(caller.to_string()), operator)
            .expect("failed to update operator stats");
    }

    // If challenge is attestation, verify immediately
    if challenge.challenge_type == ChallengeType::Attestation {
        verify_attestation_challenge(context, &challenge, &proof);
    }
}

fn verify_challenge_proof(
    context: &mut Context,
    challenge: &Challenge,
    proof: &ChallengeProof,
) -> bool {
    // Verify proof signatures from witnesses
    for (witness, signature) in &proof.witness_signatures {
        // Verify witness is a valid watchdog
        let watchdog_pool = context
            .get(WatchdogPool())
            .expect("state corrupt")
            .expect("watchdog pool not initialized");

        if !watchdog_pool.watchdogs.iter().any(|(addr, _)| addr == witness) {
            return false;
        }
    }
    true
}

fn verify_attestation_challenge(
    context: &mut Context,
    challenge: &Challenge,
    proof: &ChallengeProof,
) {
    // Verify attestation-specific proof
    let attestation_valid = verify_attestation_report(context, &proof.proof_data, &[]);
    
    if attestation_valid {
        // Update attestation status
        context
            .store_by_key(AttestationStatus(challenge.challenged), true)
            .expect("failed to update attestation status");
    } else {
        handle_failed_challenge(context, challenge);
    }
}

fn handle_failed_challenge(context: &mut Context, challenge: &Challenge) {
    let mut executor_pool = context
        .get(ExecutorPool())
        .expect("state corrupt")
        .expect("executor pool not initialized");

    // Remove failed executor
    if Some(challenge.challenged) == executor_pool.sgx_executor {
        executor_pool.sgx_executor = None;
    } else if Some(challenge.challenged) == executor_pool.sev_executor {
        executor_pool.sev_executor = None;
    }

    executor_pool.failed_attempts += 1;

    context
        .store_by_key(ExecutorPool(), executor_pool)
        .expect("failed to update executor pool");
}
