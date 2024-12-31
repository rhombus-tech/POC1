use crate::enarx::{EnarxManager, Keep, EnarxConfig, DrawbridgeToken};
use crate::types::{EnclaveType, ExecutionResult};
use crate::error::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ExecutorPool {
    enarx_manager: EnarxManager,
    sgx_executor: Option<ExecutorInstance>,
    sev_executor: Option<ExecutorInstance>,
    config: EnarxConfig,
    state: Arc<RwLock<PoolState>>,
}

struct ExecutorInstance {
    address: Address,
    keep: Keep,
    last_verified_result: Option<ExecutionResult>,
    status: ExecutorStatus,
}

#[derive(Debug)]
enum ExecutorStatus {
    Active,
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
    pub async fn new(config: EnarxConfig) -> Result<Self> {
        Ok(Self {
            enarx_manager: EnarxManager::new(config.clone()).await?,
            sgx_executor: None,
            sev_executor: None,
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
    ) -> Result<()> {
        // Launch new Keep for executor
        let keep = self.enarx_manager.launch_keep(enclave_type).await?;

        let instance = ExecutorInstance {
            address,
            keep,
            last_verified_result: None,
            status: ExecutorStatus::Active,
        };

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
    ) -> Result<ExecutionResult> {
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
    ) -> Result<ExecutionResult> {
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

    async fn get_active_executors(&self) -> Result<(&ExecutorInstance, &ExecutorInstance)> {
        match (&self.sgx_executor, &self.sev_executor) {
            (Some(sgx), Some(sev)) => {
                if sgx.status == ExecutorStatus::Active && sev.status == ExecutorStatus::Active {
                    Ok((sgx, sev))
                } else {
                    Err(Error::ExecutorNotActive)
                }
            }
            _ => Err(Error::ExecutorNotFound)
        }
    }
}
