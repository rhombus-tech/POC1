use wasmlanche::{public, Context, Address};
use crate::{
    types::*,
    state::*,
    core::utils::hash_message,
};

#[public]
pub fn challenge_executor(
    context: &mut Context,
    executor: Address,
    challenge_type: ChallengeType,
    challenge_ Vec<u8>,
) {
    ensure_initialized(context);
    ensure_phase(context, Phase::Executing);

    let caller = context.actor();
    let timestamp = context.timestamp();

    // Verify caller is a watchdog
    let watchdog_pool = context
        .get(WatchdogPool())
        .expect("state corrupt")
        .expect("watchdog pool not initialized");

    assert!(
        watchdog_pool.watchdogs.iter().any(|(addr, _)| *addr == caller),
        "not authorized watchdog"
    );

    // Verify target is an executor
    let executor_pool = context
        .get(ExecutorPool())
        .expect("state corrupt")
        .expect("executor pool not initialized");

    assert!(
        executor_pool.sgx_executor == Some(executor) || executor_pool.sev_executor == Some(executor),
        "target is not an executor"
    );

    // Create new challenge
    let challenge_id = context
        .get(ChallengeCount())
        .expect("state corrupt")
        .unwrap_or(0) + 1;

    let challenge = Challenge {
        id: challenge_id,
        challenger: caller,
        challenged: executor,
        challenge_type,
        challenge_data,
        response_deadline: timestamp + crate::CHALLENGE_RESPONSE_WINDOW,
        status: ChallengeStatus::Pending,
        verification_proofs: Vec::new(),
    };

    // Update challenge tracking
    let mut active_challenges = context
        .get(ActiveChallenges())
        .expect("state corrupt")
        .unwrap_or_default();
    active_challenges.push(challenge_id);

    // Store challenge state
    context
        .store((
            (Challenge(challenge_id), challenge),
            (ChallengeCount(), challenge_id),
            (ActiveChallenges(), active_challenges),
            (CurrentPhase(), Phase::ChallengeExecutor),
        ))
        .expect("failed to create challenge");

    // Update operator stats
    if let Some(mut operator) = context
        .get(OperatorData(caller.to_string()))
        .expect("state corrupt")
    {
        operator.challenges_initiated += 1;
        context
            .store_by_key(OperatorData(caller.to_string()), operator)
            .expect("failed to update operator stats");
    }
}
