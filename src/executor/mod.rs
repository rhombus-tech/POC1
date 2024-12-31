mod pool;
mod metrics;

pub use pool::ExecutorPool;
use crate::enarx::{Keep, EnarxConfig, DrawbridgeToken};
use crate::types::{EnclaveType, ExecutionResult};
use crate::error::{Error, Result};
use wasmlanche::{Context, Address};

pub struct Executor {
    keep: Keep,
    enclave_type: EnclaveType,
    drawbridge_token: DrawbridgeToken,
    active: bool,
}

impl Executor {
    pub async fn new(config: &EnarxConfig, enclave_type: EnclaveType) -> Result<Self> {
        // Initialize Enarx Keep
        let keep = Keep::new(config, enclave_type).await?;
        
        // Verify initial attestation
        let attestation = keep.verify_attestation().await?;
        assert!(attestation.valid, "Invalid attestation");
        
        // Get initial Drawbridge token
        let drawbridge_token = keep.get_drawbridge_token().await?;
        
        Ok(Self {
            keep,
            enclave_type,
            drawbridge_token,
            active: true,
        })
    }

    pub async fn execute(
        &mut self,
        context: &Context,
        execution_id: u128,
        payload: Vec<u8>,
    ) -> Result<ExecutionResult> {
        // Check Keep status
        self.verify_keep_status(context).await?;
        
        // Execute in Keep and get proof
        let (result, proof) = self.keep.execute_and_prove(payload).await?;
        
        Ok(ExecutionResult {
            execution_id,
            result,
            proof,
            enclave_type: self.enclave_type,
            timestamp: context.timestamp(),
            block_height: context.block_height(),
            drawbridge_token: self.drawbridge_token.clone(),
        })
    }

    async fn verify_keep_status(&mut self, context: &Context) -> Result<()> {
        // Verify health
        let health = self.keep.health_check().await?;
        if !health.healthy {
            self.active = false;
            return Err(Error::UnhealthyKeep);
        }
        
        // Refresh token if needed
        if self.drawbridge_token.is_expired(context.timestamp()) {
            self.drawbridge_token = self.keep.get_drawbridge_token().await?;
        }
        
        // Verify attestation
        let attestation = self.keep.verify_attestation().await?;
        if !attestation.valid {
            self.active = false;
            return Err(Error::InvalidAttestation);
        }
        
        Ok(())
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn enclave_type(&self) -> EnclaveType {
        self.enclave_type
    }

    pub fn keep(&self) -> &Keep {
        // Provide access to Keep for health checks etc.
        &self.keep
    }
}
