use wasmlanche::{public, Context};
use crate::{
    types::*,
    state::*,
};

#[public]
pub fn verify_challenge_response(
    context: &mut Context,
    challenge_id: u128,
    verification_result: bool,
    verification_proof: Vec<u8>,
) {
    ensure_initialized(context);
    
    let caller = context.actor();
    
    // Verify caller is a watchdog
    let watchdog_pool = context
        .get(WatchdogPool())
        .expect("state corrupt")
        .expect("watchdog pool not initialized");

    assert!(
        watchdog_pool.watchdogs.iter().any(|(addr, _)| *addr == caller),
        "not authorized watchdog"
    );

    // Get and verify challenge
    let mut challenge = context
        .get(Challenge(challenge_id))
        .expect("state corrupt")
        .expect("challenge not found");

    assert!(
        challenge.status == ChallengeStatus::Responded,
        "challenge not in response phase"
    );

    // Add verification proof
    challenge.verification_proofs.push(verification_proof);

    // Check if we have enough verifications
    let required_verifications = (watchdog_pool.watchdogs.len() * 2) / 3 + 1;
    if challenge.verification_proofs.len() >= required_verifications {
        // Process verification result
        if verification_result {
            challenge.status = ChallengeStatus::Verified;
            transition_to_executing(context);
        } else {
            challenge.status = ChallengeStatus::Failed;
            handle_challenge_failure(context, &challenge);
        }
    }

    // Store updated challenge
    context
        .store_by_key(Challenge(challenge_id), challenge)
        .expect("failed to update challenge");
}

fn handle_challenge_failure(context: &mut Context, challenge: &Challenge) {
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

    // Store updated pool
    context
        .store_by_key(ExecutorPool(), executor_pool)
        .expect("failed to update executor pool");

    // If no executors remain, transition to crashed phase
    if executor_pool.sgx_executor.is_none() && executor_pool.sev_executor.is_none() {
        context
            .store_by_key(CurrentPhase(), Phase::Crashed)
            .expect("failed to update phase");
    }
}

#[public]
pub fn get_challenge_stats(context: &mut Context) -> (u128, usize, usize, usize) {
    ensure_initialized(context);

    let total_challenges = context
        .get(ChallengeCount())
        .expect("state corrupt")
        .unwrap_or(0);

    let active_challenges = context
        .get(ActiveChallenges())
        .expect("state corrupt")
        .unwrap_or_default();

    let mut pending = 0;
    let mut verified = 0;
    let mut failed = 0;

    for challenge_id in active_challenges.iter() {
        if let Some(challenge) = context
            .get(Challenge(*challenge_id))
            .expect("state corrupt")
        {
            match challenge.status {
                ChallengeStatus::Pending => pending += 1,
                ChallengeStatus::Verified => verified += 1,
                ChallengeStatus::Failed => failed += 1,
                _ => {}
            }
        }
    }

    (total_challenges, pending, verified, failed)
}
