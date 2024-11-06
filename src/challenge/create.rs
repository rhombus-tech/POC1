use wasmlanche::{public, Context, Address};
use crate::types::{Challenge, ChallengeType, ChallengeStatus, ChallengeEvidence};

#[public]
pub async fn challenge_executor(
    context: &mut Context,
    executor: Address,
    challenge_type: ChallengeType,
    evidence_requirements: ChallengeEvidence,
) -> Result<Challenge, Error> {
    let caller = context.actor();
    ensure_watchdog(context, caller)?;

    // Create challenge with Enarx-specific requirements
    let challenge = match evidence_requirements {
        ChallengeEvidence::AttestationEvidence { .. } => {
            Challenge {
                id: generate_challenge_id(),
                challenger: caller,
                challenged: executor,
                challenge_type: ChallengeType::Attestation,
                requirements: ChallengeRequirements::Attestation {
                    required_tcb_level: Some("latest".to_string()),
                    verify_drawbridge: true,
                    verify_health: true,
                },
                status: ChallengeStatus::Pending,
                deadline: context.timestamp() + CHALLENGE_TIMEOUT,
            }
        },
        ChallengeEvidence::ExecutionEvidence { .. } => {
            Challenge {
                id: generate_challenge_id(),
                challenger: caller,
                challenged: executor,
                challenge_type: ChallengeType::Execution,
                requirements: ChallengeRequirements::Execution {
                    verify_measurement: true,
                    verify_proof: true,
                },
                status: ChallengeStatus::Pending,
                deadline: context.timestamp() + CHALLENGE_TIMEOUT,
            }
        },
    };

    // Store challenge
    store_challenge(context, &challenge)?;

    Ok(challenge)
}

fn ensure_watchdog(context: &Context, address: Address) -> Result<(), Error> {
    let watchdog_pool = context
        .get(WatchdogPool())
        .expect("state corrupt")
        .ok_or(Error::StateError("watchdog pool not initialized"))?;

    if !watchdog_pool.contains(&address) {
        return Err(Error::Unauthorized("not a watchdog".into()));
    }

    Ok(())
}

#[derive(Debug)]
pub enum Error {
    StateError(String),
    Unauthorized(String),
    StorageError(String),
}

