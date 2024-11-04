use crate::enarx::{Keep, EnarxConfig, DrawbridgeToken};
use crate::types::EnclaveType;
use wasmlanche::{Context, Address};

pub struct Executor {
    keep: Keep,
    enclave_type: EnclaveType,
    drawbridge_token: DrawbridgeToken,
    state: ExecutorState,
}

#[derive(Debug)]
struct ExecutorState {
    last_verified_height: u64,
    cached_state: Option<Vec<u8>>,
    active: bool,
}

impl Executor {
    pub fn new(config: EnarxConfig, enclave_type: EnclaveType) -> Result<Self, Error> {
        // Initialize Enarx Keep
        let keep = Keep::new(&config, enclave_type)?;
        
        // Verify attestation
        let attestation_result = keep.verify_attestation()?;
        assert!(attestation_result.valid, "Invalid attestation");

        // Get Drawbridge token
        let drawbridge_token = keep.get_drawbridge_token()?;

        Ok(Self {
            keep,
            enclave_type,
            drawbridge_token,
            state: ExecutorState {
                last_verified_height: 0,
                cached_state: None,
                active: true,
            },
        })
    }

    pub fn execute(
        &mut self,
        context: &mut Context,
        execution_id: u128,
        payload: Vec<u8>,
    ) -> Result<ExecutionResult, Error> {
        // Verify Keep is still valid
        self.verify_keep_status(context)?;

        // Execute in Keep
        let result = self.execute_in_keep(payload)?;

        // Create execution result with attestation
        let execution_result = ExecutionResult {
            execution_id,
            result_hash: result.hash(),
            keep_id: self.keep.id.clone(),
            attestation: self.keep.verify_attestation()?,
            drawbridge_token: self.drawbridge_token.clone(),
            timestamp: context.timestamp(),
        };

        Ok(execution_result)
    }

    fn verify_keep_status(&mut self, context: &mut Context) -> Result<(), Error> {
        // Verify Drawbridge token
        if self.drawbridge_token.is_expired(context.timestamp()) {
            // Refresh token
            self.drawbridge_token = self.keep.get_drawbridge_token()?;
        }

        // Verify attestation is still valid
        let attestation = self.keep.verify_attestation()?;
        if !attestation.valid {
            self.state.active = false;
            return Err(Error::InvalidAttestation);
        }

        Ok(())
    }

    fn execute_in_keep(&mut self, payload: Vec<u8>) -> Result<KeepExecutionResult, Error> {
        // Execute payload in Enarx Keep
        let result = self.keep.execute(payload)?;
        
        // Update cached state if needed
        if let Some(new_state) = result.state_update {
            self.state.cached_state = Some(new_state);
            self.state.last_verified_height = result.height;
        }

        Ok(result)
    }
}
