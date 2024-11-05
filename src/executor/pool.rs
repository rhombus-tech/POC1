use crate::enarx::{EnarxManager, Keep, EnarxConfig};
use crate::types::{EnclaveType, ExecutionResult};
use std::collections::HashMap;
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
            timestamp: std::time::SystemTime::now(),
            enclave_type: instance.keep.enclave_type(),
            drawbridge_token: token,
        })
    }

    pub async fn handle_challenge(
        &mut self,
        challenge: Challenge,
    ) -> Result<(), Error> {
        let challenged_executor = match challenge.challenge_type {
            ChallengeType::Attestation | ChallengeType::Execution => {
                self.get_executor_by_address(challenge.challenged)?
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
                ).await?;
            }
            ChallengeType::Execution => {
                self.handle_execution_challenge(
                    challenged_executor,
                    &challenge,
                ).await?;
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
        // Create challenges for both executors
        let sgx_challenge = self.create_execution_challenge(
            execution_id,
            EnclaveType::IntelSGX,
        ).await?;

        let sev_challenge = self.create_execution_challenge(
            execution_id,
            EnclaveType::AMDSEV,
        ).await?;

        // Store challenges
        self.store_challenges(&[sgx_challenge, sev_challenge]).await?;

        Ok(())
    }
}
