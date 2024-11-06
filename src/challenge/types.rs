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

#[derive(Debug)]
pub enum ChallengeEvidence {
    AttestationEvidence {
        attestation_report: AttestationReport,
        drawbridge_token: DrawbridgeToken,
        keep_health: KeepHealth,
    },
    ExecutionEvidence {
        result_hash: Vec<u8>,
        execution_proof: Vec<u8>,
        keep_measurement: Vec<u8>,
    },
}

#[derive(Debug)]
pub struct Challenge {
    pub id: u128,
    pub challenger: Address,
    pub challenged: Address,
    pub challenge_type: ChallengeType,
    pub execution_id: Option<u128>,
    pub timestamp: SystemTime,
    pub deadline: SystemTime,
    pub status: ChallengeStatus,
}

pub const MIN_VERIFICATION_PROOFS: usize = 3;
