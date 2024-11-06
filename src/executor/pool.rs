use crate::enarx::{EnarxManager, Keep, EnarxConfig, DrawbridgeToken};
use crate::types::{EnclaveType, ExecutionResult};
use crate::challenge::{Challenge, ChallengeType, ChallengeStatus, ChallengeEvidence};
use crate::error::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, Duration};
use tokio::sync::RwLock;


pub struct ExecutorPool {
    enarx_manager: EnarxManager,
    sgx_executor: Option<ExecutorInstance>,
    sev_executor: Option<ExecutorInstance>,
    watchdog_verifiers: HashMap<Address, WatchdogVerifier>,
    config: EnarxConfig,
    state: Arc<RwLock<PoolState>>,
}

struct ExecutorInstance {
    address: Address,
    keep: Keep,
    last_verified_result: Option<ExecutionResult>,
    status: ExecutorStatus,
}

struct WatchdogVerifier {
    address: Address,
    enclave_type: EnclaveType,
    verification_count: u64,
    last_challenge: Option<u128>,
}

#[derive(Debug)]
enum ExecutorStatus {
    Active,
    Challenged,
    PendingReplacement,
    Failed,
}

#[derive(Debug)]
struct PoolState {
    execution_count: u64,
    last_sync_height: u64,
    verification_results: HashMap<u128, VerificationPair>,
}

#[derive(Debug)]
struct VerificationPair {
    sgx_result: Option<ExecutionResult>,
    sev_result: Option<ExecutionResult>,
    verified: bool,
}

#[derive(Debug)]
pub enum Error {
    ExecutorNotFound,
    UnhealthyKeep,
    ExecutionFailed(String),
    VerificationFailed(String),
    ChallengeError(String),
    ExecutionNotFound,
    InvalidEvidence,
    EnarxError(String),
}

impl ExecutorPool {
    pub async fn new(config: EnarxConfig) -> Result<Self, Error> {
        Ok(Self {
            enarx_manager: EnarxManager::new(config.clone()).await?,
            sgx_executor: None,
            sev_executor: None,
            watchdog_verifiers: HashMap::new(),
            config,
            state: Arc::new(RwLock::new(PoolState {
                execution_count: 0,
                last_sync_height: 0,
                verification_results: HashMap::new(),
            })),
        })
    }

    pub async fn register_executor(
        &mut self,
        address: Address,
        enclave_type: EnclaveType,
    ) -> Result<(), Error> {
        // Launch new Keep for executor
        let keep = self.enarx_manager.launch_keep(enclave_type).await?;

        let instance = ExecutorInstance {
            address,
            keep,
            last_verified_result: None,
            status: ExecutorStatus::Active,
        };

        // Register in appropriate slot
        match enclave_type {
            EnclaveType::IntelSGX => {
                assert!(self.sgx_executor.is_none(), "SGX executor already registered");
                self.sgx_executor = Some(instance);
            }
            EnclaveType::AMDSEV => {
                assert!(self.sev_executor.is_none(), "SEV executor already registered");
                self.sev_executor = Some(instance);
            }
        }

        Ok(())
    }

    pub async fn execute(
        &mut self,
        execution_id: u128,
        payload: Vec<u8>,
    ) -> Result<ExecutionResult, Error> {
        // Ensure both executors are available
        let (sgx_executor, sev_executor) = self.get_active_executors()?;

        // Execute on both SGX and SEV
        let (sgx_result, sev_result) = tokio::join!(
            self.execute_on_instance(sgx_executor, execution_id, payload.clone()),
            self.execute_on_instance(sev_executor, execution_id, payload),
        );

        // Store results for verification
        let mut state = self.state.write().await;
        state.verification_results.insert(
            execution_id,
            VerificationPair {
                sgx_result: Some(sgx_result?),
                sev_result: Some(sev_result?),
                verified: false,
            },
        );

        // Return SGX result (primary)
        Ok(sgx_result?)
    }

    async fn execute_on_instance(
        &self,
        instance: &ExecutorInstance,
        execution_id: u128,
        payload: Vec<u8>,
    ) -> Result<ExecutionResult, Error> {
        // Verify Keep health before execution
        let health = instance.keep.health_check().await?;
        if !self.enarx_manager.verify_keep_health(&health) {
            return Err(Error::UnhealthyKeep);
        }

        // Get fresh Drawbridge token
        let token = instance.keep.get_drawbridge_token().await?;

        // Execute with verification
        let result = instance.keep.execute_with_verification(payload).await?;

        Ok(ExecutionResult {
            execution_id,
            result,
            keep_id: instance.keep.id().to_string(),
            timestamp: SystemTime::now(),
            enclave_type: instance.keep.enclave_type(),
            drawbridge_token: token,
        })
    }

pub async fn handle_challenge(
    &mut self,
    challenge: Challenge,
) -> Result<()> {  // Just Result<()> instead of Result<(), Error>
    let challenged_executor = match challenge.challenge_type {
        ChallengeType::Attestation | ChallengeType::Execution => {
            self.get_executor_by_address(challenge.challenged)
                .map_err(|_| Error::ExecutorNotFound)?  // Using centralized Error type
        }
    };

    // Update executor status
    challenged_executor.status = ExecutorStatus::Challenged;

    // Handle based on challenge type
    match challenge.challenge_type {
        ChallengeType::Attestation => {
            self.handle_attestation_challenge(
                challenged_executor,
                &challenge,
            ).await
            .map_err(|e| Error::challenge_error(format!("Attestation challenge failed: {}", e)))?
        }
        ChallengeType::Execution => {
            self.handle_execution_challenge(
                challenged_executor,
                &challenge,
            ).await
            .map_err(|e| Error::challenge_error(format!("Execution challenge failed: {}", e)))?
        }
    }

    Ok(())
}
    async fn handle_attestation_challenge(
        &mut self,
        executor: &mut ExecutorInstance,
        challenge: &Challenge,
    ) -> Result<(), Error> {
        // Get fresh attestation
        let attestation = executor.keep.verify_attestation().await?;
        
        // Get current token
        let token = executor.keep.get_drawbridge_token().await?;
        
        // Get Keep health
        let health = executor.keep.health_check().await?;

        // Create evidence
        let evidence = ChallengeEvidence::AttestationEvidence {
            attestation_report: attestation,
            drawbridge_token: token,
            keep_health: health,
        };

        // Submit evidence
        self.submit_challenge_evidence(challenge.id, evidence).await?;

        Ok(())
    }

    async fn handle_execution_challenge(
        &mut self,
        executor: &mut ExecutorInstance,
        challenge: &Challenge,
    ) -> Result<(), Error> {
        // Get execution results
        let state = self.state.read().await;
        let verification_pair = state.verification_results
            .get(&challenge.execution_id)
            .ok_or(Error::ExecutionNotFound)?;

        // Get Keep measurement
        let measurement = executor.keep.get_measurement().await?;

        // Create evidence
        let evidence = ChallengeEvidence::ExecutionEvidence {
            result_hash: verification_pair.get_result_for_type(
                executor.keep.enclave_type(),
            )?.result_hash.clone(),
            execution_proof: executor.keep.get_execution_proof().await?,
            keep_measurement: measurement,
        };

        // Submit evidence
        self.submit_challenge_evidence(challenge.id, evidence).await?;

        Ok(())
    }

    async fn submit_challenge_evidence(
        &mut self,
        challenge_id: u128,
        evidence: ChallengeEvidence,
    ) -> Result<(), Error> {
        // Verify evidence is valid
        self.verify_evidence(&evidence).await?;

        // Store evidence for verification
        self.store_challenge_evidence(challenge_id, evidence).await?;

        Ok(())
    }

    pub async fn verify_execution_results(
        &mut self,
        execution_id: u128,
    ) -> Result<bool, Error> {
        let mut state = self.state.write().await;
        let pair = state.verification_results
            .get_mut(&execution_id)
            .ok_or(Error::ExecutionNotFound)?;

        // Ensure we have both results
        let (sgx_result, sev_result) = match (pair.sgx_result.as_ref(), pair.sev_result.as_ref()) {
            (Some(sgx), Some(sev)) => (sgx, sev),
            _ => return Ok(false),
        };

        // Verify results match
        let verified = sgx_result.result_hash == sev_result.result_hash;
        pair.verified = verified;

        if !verified {
            // Trigger challenge process
            self.initiate_mismatch_challenge(execution_id).await?;
        }

        Ok(verified)
    }

    async fn initiate_mismatch_challenge(
        &mut self,
        execution_id: u128,
    ) -> Result<(), Error> {
        // Get both results
        let state = self.state.read().await;
        let pair = state.verification_results
            .get(&execution_id)
            .ok_or(Error::ExecutionNotFound)?;

        // Create challenges for both executors
        let sgx_challenge = self.create_execution_challenge(
            execution_id,
            EnclaveType::IntelSGX,
            pair.sgx_result.as_ref().unwrap().result_hash.clone(),
        ).await?;

        let sev_challenge = self.create_execution_challenge(
            execution_id,
            EnclaveType::AMDSEV,
            pair.sev_result.as_ref().unwrap().result_hash.clone(),
        ).await?;

        // Store challenges
        self.store_challenges(&[sgx_challenge, sev_challenge]).await?;

        Ok(())
    }

    async fn create_execution_challenge(
        &self,
        execution_id: u128,
        enclave_type: EnclaveType,
        result_hash: Vec<u8>,
    ) -> Result<Challenge, Error> {
        let executor = match enclave_type {
            EnclaveType::IntelSGX => self.sgx_executor.as_ref(),
            EnclaveType::AMDSEV => self.sev_executor.as_ref(),
        }.ok_or(Error::ExecutorNotFound)?;

        Ok(Challenge {
            id: generate_challenge_id(),
            challenger: self.select_challenger(enclave_type)?,
            challenged: executor.address,
            challenge_type: ChallengeType::Execution,
            execution_id,
            result_hash,
            timestamp: SystemTime::now(),
            deadline: SystemTime::now() + Duration::from_secs(300), // 5 minute deadline
            status: ChallengeStatus::Pending,
        })
    }

    fn select_challenger(&self, enclave_type: EnclaveType) -> Result<Address, Error> {
        // Select a watchdog of the same enclave type
        self.watchdog_verifiers.iter()
            .find(|(_, w)| w.enclave_type == enclave_type)
            .map(|(addr, _)| *addr)
            .ok_or(Error::NoAvailableWatchdog)
    }

    async fn verify_evidence(
        &self,
        evidence: &ChallengeEvidence,
    ) -> Result<(), Error> {
        match evidence {
            ChallengeEvidence::AttestationEvidence {
                attestation_report,
                drawbridge_token,
                keep_health,
            } => {
                // Verify attestation
                if !self.verify_attestation_report(attestation_report).await? {
                    return Err(Error::InvalidEvidence);
                }

                // Verify Drawbridge token
                if !self.verify_drawbridge_token(drawbridge_token).await? {
                    return Err(Error::InvalidEvidence);
                }

                // Verify Keep health
                if !self.verify_keep_health(keep_health).await? {
                    return Err(Error::InvalidEvidence);
                }
            },
            ChallengeEvidence::ExecutionEvidence {
                result_hash,
                execution_proof,
                keep_measurement,
            } => {
                // Verify execution proof
                if !self.verify_execution_proof(execution_proof, result_hash).await? {
                    return Err(Error::InvalidEvidence);
                }

                // Verify Keep measurement
                if !self.verify_keep_measurement(keep_measurement).await? {
                    return Err(Error::InvalidEvidence);
                }
            },
        }

        Ok(())
    }

// Helper functions
    async fn get_active_executors(&self) -> Result<(&ExecutorInstance, &ExecutorInstance), Error> {
        match (&self.sgx_executor, &self.sev_executor) {
            (Some(sgx), Some(sev)) => {
                // Verify both executors are active
                if sgx.status != ExecutorStatus::Active || sev.status != ExecutorStatus::Active {
                    return Err(Error::ExecutorNotActive);
                }
                Ok((sgx, sev))
            }
            _ => Err(Error::ExecutorNotFound),
        }
    }

    fn get_executor_by_address(&mut self, address: Address) -> Result<&mut ExecutorInstance, Error> {
        if let Some(ref mut sgx) = self.sgx_executor {
            if sgx.address == address {
                return Ok(sgx);
            }
        }
        if let Some(ref mut sev) = self.sev_executor {
            if sev.address == address {
                return Ok(sev);
            }
        }
        Err(Error::ExecutorNotFound)
    }

    async fn store_challenges(&mut self, challenges: &[Challenge]) -> Result<(), Error> {
        for challenge in challenges {
            self.store_challenge(challenge.clone()).await?;
        }
        Ok(())
    }

    async fn store_challenge(&mut self, challenge: Challenge) -> Result<(), Error> {
        // Store challenge in state
        let mut state = self.state.write().await;
        // Implementation depends on your storage mechanism
        Ok(())
    }

    async fn store_challenge_evidence(
        &mut self,
        challenge_id: u128,
        evidence: ChallengeEvidence,
    ) -> Result<(), Error> {
        // Store evidence in state
        let mut state = self.state.write().await;
        // Implementation depends on your storage mechanism
        Ok(())
    }

    async fn verify_attestation_report(
        &self,
        report: &AttestationReport,
    ) -> Result<bool, Error> {
        // Implement attestation verification logic
        Ok(true)
    }

    async fn verify_drawbridge_token(
        &self,
        token: &DrawbridgeToken,
    ) -> Result<bool, Error> {
        // Implement token verification logic
        Ok(true)
    }

    async fn verify_keep_health(
        &self,
        health: &KeepHealth,
    ) -> Result<bool, Error> {
        // Implement health verification logic
        Ok(true)
    }

    async fn verify_execution_proof(
        &self,
        proof: &[u8],
        result_hash: &[u8],
    ) -> Result<bool, Error> {
        // Implement proof verification logic
        Ok(true)
    }

    async fn verify_keep_measurement(
        &self,
        measurement: &[u8],
    ) -> Result<bool, Error> {
        // Implement measurement verification logic
        Ok(true)
    }

    fn generate_challenge_id() -> u128 {
        use rand::Rng;
        rand::thread_rng().gen()
    }
}

// Error Implementation
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ExecutorNotFound => write!(f, "Executor not found"),
            Error::UnhealthyKeep => write!(f, "Unhealthy Keep"),
            Error::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            Error::VerificationFailed(msg) => write!(f, "Verification failed: {}", msg),
            Error::ChallengeError(msg) => write!(f, "Challenge error: {}", msg),
            Error::ExecutionNotFound => write!(f, "Execution not found"),
            Error::InvalidEvidence => write!(f, "Invalid evidence"),
            Error::EnarxError(msg) => write!(f, "Enarx error: {}", msg),
            Error::ExecutorNotActive => write!(f, "Executor not active"),
            Error::NoAvailableWatchdog => write!(f, "No available watchdog"),
        }
    }
}

impl std::error::Error for Error {}

// Tests
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    async fn setup_test_pool() -> ExecutorPool {
        let config = EnarxConfig {
            // Test configuration
            keep_binary: "test_binary".into(),
            attestation_config: Default::default(),
            drawbridge_config: Default::default(),
        };
        ExecutorPool::new(config).await.unwrap()
    }

    #[test]
    async fn test_executor_registration() {
        let mut pool = setup_test_pool().await;
        
        // Register SGX executor
        pool.register_executor(
            Address::from([1u8; 32]),
            EnclaveType::IntelSGX,
        ).await.unwrap();

        // Register SEV executor
        pool.register_executor(
            Address::from([2u8; 32]),
            EnclaveType::AMDSEV,
        ).await.unwrap();

        assert!(pool.sgx_executor.is_some());
        assert!(pool.sev_executor.is_some());
    }

    #[test]
    async fn test_execution_verification() {
        let mut pool = setup_test_pool().await;
        
        // Setup executors
        let sgx_addr = Address::from([1u8; 32]);
        let sev_addr = Address::from([2u8; 32]);
        
        pool.register_executor(sgx_addr, EnclaveType::IntelSGX).await.unwrap();
        pool.register_executor(sev_addr, EnclaveType::AMDSEV).await.unwrap();

        // Execute
        let execution_id = 1u128;
        let payload = vec![1, 2, 3];
        
        let result = pool.execute(execution_id, payload).await.unwrap();
        
        // Verify results match
        assert!(pool.verify_execution_results(execution_id).await.unwrap());
    }

    #[test]
    async fn test_challenge_handling() {
        let mut pool = setup_test_pool().await;
        
        // Setup system
        let sgx_addr = Address::from([1u8; 32]);
        pool.register_executor(sgx_addr, EnclaveType::IntelSGX).await.unwrap();

        // Create challenge
        let challenge = Challenge {
            id: 1u128,
            challenger: Address::from([3u8; 32]),
            challenged: sgx_addr,
            challenge_type: ChallengeType::Attestation,
            execution_id: None,
            timestamp: SystemTime::now(),
            deadline: SystemTime::now() + Duration::from_secs(300),
            status: ChallengeStatus::Pending,
        };

        // Handle challenge
        pool.handle_challenge(challenge).await.unwrap();

        // Verify executor status
        let executor = pool.get_executor_by_address(sgx_addr).unwrap();
        assert!(matches!(executor.status, ExecutorStatus::Challenged));
    }

    #[test]
    async fn test_mismatch_handling() {
        let mut pool = setup_test_pool().await;
        
        // Setup system with mismatched results
        // ... test implementation ...
    }

    // Add more tests as needed...
}
