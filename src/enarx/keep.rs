use enarx_keep_api::{self, Keep as EnarxKeep, KeepConfig, KeepStatus};
use crate::types::EnclaveType;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Keep {
    id: String,
    enclave_type: EnclaveType,
    keep: Arc<RwLock<EnarxKeep>>,
    config: KeepConfig,
    status: KeepStatus,
}

impl Keep {
    pub async fn new(config: &KeepConfig, enclave_type: EnclaveType) -> Result<Self, Error> {
        // Configure Keep backend based on enclave type
        let backend = match enclave_type {
            EnclaveType::IntelSGX => "sgx",
            EnclaveType::AMDSEV => "sev",
        };

        // Create Keep configuration
        let keep_config = KeepConfig {
            backend: backend.to_string(),
            binary: config.binary.clone(),
            attestation_config: config.attestation_config.clone(),
            heap_size: config.heap_size,
            stack_size: config.stack_size,
            debug: config.debug,
        };

        // Launch Keep
        let keep = EnarxKeep::launch(&keep_config).await?;
        let keep_id = keep.id().to_string();

        Ok(Self {
            id: keep_id,
            enclave_type,
            keep: Arc::new(RwLock::new(keep)),
            config: keep_config,
            status: KeepStatus::Launched,
        })
    }

    /// Lifecycle Management Methods

    pub async fn start(&mut self) -> Result<(), Error> {
        let mut keep = self.keep.write().await;
        keep.start().await?;
        self.status = KeepStatus::Running;
        Ok(())
    }

    pub async fn pause(&mut self) -> Result<(), Error> {
        let mut keep = self.keep.write().await;
        keep.pause().await?;
        self.status = KeepStatus::Paused;
        Ok(())
    }

    pub async fn resume(&mut self) -> Result<(), Error> {
        let mut keep = self.keep.write().await;
        keep.resume().await?;
        self.status = KeepStatus::Running;
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<(), Error> {
        let mut keep = self.keep.write().await;
        keep.shutdown().await?;
        self.status = KeepStatus::Shutdown;
        Ok(())
    }

    /// Health Check and Monitoring

    pub async fn health_check(&self) -> Result<KeepHealth, Error> {
        let keep = self.keep.read().await;
        let status = keep.check_status().await?;
        let memory = keep.memory_usage().await?;
        let attestation = keep.get_attestation().await?;

        Ok(KeepHealth {
            status,
            memory_usage: memory,
            last_attestation: attestation.timestamp,
            keep_id: self.id.clone(),
        })
    }

    pub async fn refresh_attestation(&mut self) -> Result<(), Error> {
        let mut keep = self.keep.write().await;
        keep.refresh_attestation().await?;
        Ok(())
    }

    /// State Management

    pub async fn backup_state(&self) -> Result<KeepState, Error> {
        let keep = self.keep.read().await;
        let state = keep.export_state().await?;
        
        Ok(KeepState {
            keep_id: self.id.clone(),
            state_ state,
            timestamp: std::time::SystemTime::now(),
        })
    }

    pub async fn restore_state(&mut self, state: KeepState) -> Result<(), Error> {
        if state.keep_id != self.id {
            return Err(Error::InvalidState("Keep ID mismatch".into()));
        }

        let mut keep = self.keep.write().await;
        keep.import_state(&state.state_data).await?;
        Ok(())
    }

    /// Execution

    pub async fn execute(&self, payload: Vec<u8>) -> Result<Vec<u8>, Error> {
        let keep = self.keep.read().await;
        
        if self.status != KeepStatus::Running {
            return Err(Error::InvalidState("Keep is not running".into()));
        }

        Ok(keep.execute(payload).await?)
    }

    /// Migration Support

    pub async fn prepare_migration(&mut self) -> Result<MigrationPackage, Error> {
        let mut keep = self.keep.write().await;
        let state = keep.export_state().await?;
        let attestation = keep.get_attestation().await?;

        Ok(MigrationPackage {
            keep_id: self.id.clone(),
            state,
            attestation,
            config: self.config.clone(),
            timestamp: std::time::SystemTime::now(),
        })
    }

    pub async fn receive_migration(
        config: &KeepConfig,
        package: MigrationPackage,
    ) -> Result<Self, Error> {
        // Create new Keep
        let mut keep = Self::new(config, package.config.backend.into()).await?;
        
        // Import state
        keep.restore_state(KeepState {
            keep_id: package.keep_id,
            state_ package.state,
            timestamp: package.timestamp,
        }).await?;

        Ok(keep)
    }
}

#[derive(Debug)]
pub struct KeepHealth {
    pub status: KeepStatus,
    pub memory_usage: MemoryStats,
    pub last_attestation: u64,
    pub keep_id: String,
}

#[derive(Debug)]
pub struct KeepState {
    pub keep_id: String,
    pub state_ Vec<u8>,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug)]
pub struct MigrationPackage {
    pub keep_id: String,
    pub state: Vec<u8>,
    pub attestation: enarx_keep_api::Attestation,
    pub config: KeepConfig,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug)]
pub enum Error {
    LaunchFailed(enarx_keep_api::Error),
    AttestationFailed(String),
    ExecutionFailed(String),
    InvalidState(String),
    MigrationFailed(String),
}

// Usage example in executor:
impl Executor {
    pub async fn new(config: KeepConfig, enclave_type: EnclaveType) -> Result<Self, Error> {
        let keep = Keep::new(&config, enclave_type).await?;
        keep.start().await?;

        Ok(Self {
            keep,
            enclave_type,
            // ... other fields
        })
    }

    pub async fn execute_with_lifecycle(
        &self,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, Error> {
        // Health check before execution
        let health = self.keep.health_check().await?;
        if health.status != KeepStatus::Running {
            return Err(Error::ExecutionFailed("Keep not in running state".into()));
        }

        // Execute
        let result = self.keep.execute(payload).await?;

        // Periodic attestation refresh if needed
        if self.should_refresh_attestation(&health) {
            self.keep.refresh_attestation().await?;
        }

        Ok(result)
    }
}
