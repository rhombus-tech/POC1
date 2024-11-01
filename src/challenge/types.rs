use crate::types::*;

#[derive(Debug)]
pub struct ChallengeContext {
    pub challenge_id: u128,
    pub current_phase: Phase,
    pub proof_count: usize,
    pub required_verifications: usize,
}

#[derive(Debug)]
pub struct ChallengeResult {
    pub success: bool,
    pub new_phase: Phase,
    pub verification_ Vec<u8>,
}

pub const MIN_VERIFICATION_PROOFS: usize = 3;
