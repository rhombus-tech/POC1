use enarx_keep_api::{self, Keep as EnarxKeep, KeepConfig, KeepStatus};
use crate::types::EnclaveType;
use crate::error::{Error, Result};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Keep {
    id: String,
    enclave_type: EnclaveType,
    keep: Arc<RwLock<EnarxKeep>>,
    config: KeepConfig,
    status: KeepStatus,
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
    pub state_data: Vec<u8>,
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
pub struct MemoryStats {
    pub used: usize,
    pub total: usize,
}

impl Keep {
    pub async fn new(config: &KeepConfig, enclave_type: EnclaveType) -> Result<Self> {
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

    pub async fn start(&mut self) -> Result<()> {
        let mut keep = self.keep.write().await;
        keep.start().await?;
        self.status = KeepStatus::Running;
        Ok(())
    }

    pub async fn pause(&mut self) -> Result<()> {
        let mut keep = self.keep.write().await;
        keep.pause().await?;
        self.status = KeepStatus::Paused;
        Ok(())
    }

    pub async fn resume(&mut self) -> Result<()> {
        let mut keep = self.keep.write().await;
        keep.resume().await?;
        self.status = KeepStatus::Running;
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        let mut keep = self.keep.write().await;
        keep.shutdown().await?;
        self.status = KeepStatus::Shutdown;
        Ok(())
    }

    /// Health Check and Monitoring

    pub async fn health_check(&self) -> Result<KeepHealth> {
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

    pub async fn refresh_attestation(&mut self) -> Result<()> {
        let mut keep = self.keep.write().await;
        keep.refresh_attestation().await?;
        Ok(())
    }

    /// State Management

    pub async fn backup_state(&self) -> Result<KeepState> {
        let keep = self.keep.read().await;
        let state = keep.export_state().await?;
        
        Ok(KeepState {
            keep_id: self.id.clone(),
            state_ state,
            timestamp: std::time::SystemTime::now(),
        })
    }

    pub async fn restore_state(&mut self, state: KeepState) -> Result<()> {
        if state.keep_id != self.id {
            return Err(Error::keep_error("Keep ID mismatch"));
        }

        let mut keep = self.keep.write().await;
        keep.import_state(&state.state_data).await?;
        Ok(())
    }

    /// Execution

    pub async fn execute(&self, payload: Vec<u8>) -> Result<Vec<u8>> {
        let keep = self.keep.read().await;
        
        if self.status != KeepStatus::Running {
            return Err(Error::keep_error("Keep is not running"));
        }

        Ok(keep.execute(payload).await?)
    }

    /// Migration Support

    pub async fn prepare_migration(&mut self) -> Result<MigrationPackage> {
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
    ) -> Result<Self> {
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

// Example usage in Executor
impl Executor {
    pub async fn new(config: KeepConfig, enclave_type: EnclaveType) -> Result<Self> {
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
    ) -> Result<Vec<u8>> {
        // Health check before execution
        let health = self.keep.health_check().await?;
        if health.status != KeepStatus::Running {
            return Err(Error::keep_error("Keep not in running state"));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_keep_lifecycle() -> Result<()> {
        let config = KeepConfig::default();
        let mut keep = Keep::new(&config, EnclaveType::IntelSGX).await?;

        keep.start().await?;
        assert_eq!(keep.status, KeepStatus::Running);

        keep.pause().await?;
        assert_eq!(keep.status, KeepStatus::Paused);

        keep.resume().await?;
        assert_eq!(keep.status, KeepStatus::Running);

        keep.shutdown().await?;
        assert_eq!(keep.status, KeepStatus::Shutdown);

        Ok(())
    }

    #[tokio::test]
    async fn test_keep_state_management() -> Result<()> {
        let config = KeepConfig::default();
        let mut keep = Keep::new(&config, EnclaveType::IntelSGX).await?;
        keep.start().await?;

        let state = keep.backup_state().await?;
        keep.restore_state(state).await?;

        Ok(())
    }
}
