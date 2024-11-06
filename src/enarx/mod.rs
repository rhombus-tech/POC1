pub mod keep;
pub mod attestation;
pub mod drawbridge;

use wasmlanche::{Context, Address};
use std::path::PathBuf;
use std::time::{SystemTime, Duration};

pub use self::keep::{Keep, KeepHealth, KeepState, MigrationPackage};
pub use self::attestation::{AttestationReport, AttestationResult};
pub use self::drawbridge::{DrawbridgeToken, DrawbridgeError};

#[derive(Debug, Clone)]
pub struct EnarxConfig {
    pub keep_binary: PathBuf,
    pub attestation_config: AttestationConfig,
    pub drawbridge_config: DrawbridgeConfig,
    pub heap_size: usize,
    pub stack_size: usize,
    pub debug: bool,
}

#[derive(Debug, Clone)]
pub struct AttestationConfig {
    pub refresh_interval: Duration,
    pub required_tcb_level: Option<String>,
    pub platform_requirements: Option<PlatformRequirements>,
}

#[derive(Debug, Clone)]
pub struct DrawbridgeConfig {
    pub token_refresh_interval: Duration,
    pub verification_requirements: VerificationRequirements,
}

#[derive(Debug, Clone)]
pub struct PlatformRequirements {
    pub min_cpu_svn: Option<u64>,
    pub required_features: Vec<String>,
    pub allowed_measurements: Vec<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct VerificationRequirements {
    pub require_matching_measurements: bool,
    pub require_matching_platform: bool,
    pub max_token_age: Duration,
}

pub struct EnarxManager {
    config: EnarxConfig,
    active_keeps: Vec<ActiveKeep>,
}

struct ActiveKeep {
    keep: Keep,
    last_health_check: SystemTime,
    last_attestation_refresh: SystemTime,
    last_token_refresh: SystemTime,
}

impl EnarxManager {
    pub async fn new(config: EnarxConfig) -> Result<Self, Error> {
        Ok(Self {
            config,
            active_keeps: Vec::new(),
        })
    }

    pub async fn launch_keep(&mut self, enclave_type: EnclaveType) -> Result<Keep, Error> {
        // Create and launch new Keep
        let keep = Keep::new(&self.config, enclave_type).await?;
        
        // Initialize Keep
        keep.start().await?;
        
        // Get initial attestation
        let attestation = keep.verify_attestation().await?;
        
        // Get initial Drawbridge token
        let token = keep.get_drawbridge_token().await?;

        // Track active Keep
        self.active_keeps.push(ActiveKeep {
            keep: keep.clone(),
            last_health_check: SystemTime::now(),
            last_attestation_refresh: SystemTime::now(),
            last_token_refresh: SystemTime::now(),
        });

        Ok(keep)
    }

    pub async fn maintain_keeps(&mut self) -> Result<(), Error> {
        let now = SystemTime::now();
        
        // Check each active Keep
        for active_keep in &mut self.active_keeps {
            // Health check if needed
            if now.duration_since(active_keep.last_health_check)? >= Duration::from_secs(60) {
                let health = active_keep.keep.health_check().await?;
                active_keep.last_health_check = now;
                
                if !self.verify_keep_health(&health) {
                    // Handle unhealthy Keep
                    self.handle_unhealthy_keep(&active_keep.keep).await?;
                }
            }

            // Refresh attestation if needed
            if now.duration_since(active_keep.last_attestation_refresh)? >= self.config.attestation_config.refresh_interval {
                active_keep.keep.refresh_attestation().await?;
                active_keep.last_attestation_refresh = now;
            }

            // Refresh Drawbridge token if needed
            if now.duration_since(active_keep.last_token_refresh)? >= self.config.drawbridge_config.token_refresh_interval {
                active_keep.keep.get_drawbridge_token().await?;
                active_keep.last_token_refresh = now;
            }
        }

        Ok(())
    }

    async fn handle_unhealthy_keep(&mut self, keep: &Keep) -> Result<(), Error> {
        // Attempt recovery
        if let Err(e) = keep.restart().await {
            // If recovery fails, prepare for migration
            let migration_package = keep.prepare_migration().await?;
            
            // Launch new Keep
            let new_keep = Keep::receive_migration(&self.config, migration_package).await?;
            
            // Replace old Keep
            self.replace_keep(keep.id().to_string(), new_keep).await?;
        }

        Ok(())
    }

    async fn replace_keep(&mut self, old_id: String, new_keep: Keep) -> Result<(), Error> {
        // Find and remove old Keep
        if let Some(pos) = self.active_keeps.iter().position(|k| k.keep.id() == old_id) {
            let old_keep = &self.active_keeps[pos].keep;
            
            // Shutdown old Keep
            old_keep.shutdown().await?;
            
            // Replace with new Keep
            self.active_keeps[pos] = ActiveKeep {
                keep: new_keep,
                last_health_check: SystemTime::now(),
                last_attestation_refresh: SystemTime::now(),
                last_token_refresh: SystemTime::now(),
            };
        }

        Ok(())
    }

    fn verify_keep_health(&self, health: &KeepHealth) -> bool {
        // Check basic health
        if health.status != enarx_keep_api::KeepStatus::Running {
            return false;
        }

        // Verify memory usage
        if health.memory_usage.used > self.config.heap_size {
            return false;
        }

        // Verify attestation age
        let attestation_age = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() - health.last_attestation;
            
        if attestation_age > self.config.attestation_config.refresh_interval.as_secs() {
            return false;
        }

        true
    }
}

#[derive(Debug, Clone)]
pub struct EnarxConfig {
    pub keep_binary: PathBuf,
    pub attestation_config: AttestationConfig,
    pub drawbridge_config: DrawbridgeConfig,
    pub heap_size: usize,
    pub stack_size: usize,
    pub rotation_threshold: u64,
    pub rotation_interval: Duration,
    pub min_watchdogs: usize,
    pub watchdog_timeout: Duration,
    pub backup_validity_period: Duration,
}

#[derive(Debug, Clone, Default)]
pub struct AttestationConfig {
    pub refresh_interval: Duration,
    pub required_tcb_level: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DrawbridgeConfig {
    pub token_refresh_interval: Duration,
    pub verification_requirements: VerificationRequirements,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Keep error: {0}")]
    KeepError(#[from] keep::Error),
    
    #[error("Attestation error: {0}")]
    AttestationError(#[from] attestation::Error),
    
    #[error("Drawbridge error: {0}")]
    DrawbridgeError(#[from] drawbridge::DrawbridgeError),
    
    #[error("Time error: {0}")]
    TimeError(#[from] std::time::SystemTimeError),
}
