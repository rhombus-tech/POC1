use std::process::Command;
use crate::types::EnclaveType;

#[derive(Debug)]
pub struct Keep {
    pub id: String,
    pub enclave_type: EnclaveType,
    pub attestation_token: Vec<u8>,
    pub measurement: Vec<u8>,
}

impl Keep {
    pub fn new(config: &EnarxConfig, enclave_type: EnclaveType) -> Result<Self, Error> {
        // Launch Enarx Keep with appropriate backend
        let backend = match enclave_type {
            EnclaveType::IntelSGX => "sgx",
            EnclaveType::AMDSEV => "sev",
        };

        let output = Command::new("enarx")
            .arg("run")
            .arg("--backend")
            .arg(backend)
            .arg(&config.keep_binary)
            .output()?;

        // Parse Keep ID and attestation data
        let keep_id = parse_keep_id(&output.stdout)?;
        let attestation_token = get_attestation_token(keep_id.clone())?;
        let measurement = get_keep_measurement(keep_id.clone())?;

        Ok(Self {
            id: keep_id,
            enclave_type,
            attestation_token,
            measurement,
        })
    }

    pub fn verify_attestation(&self) -> Result<AttestationResult, Error> {
        attestation::verify_keep(
            &self.attestation_token,
            &self.measurement,
            self.enclave_type,
        )
    }

    pub fn get_drawbridge_token(&self) -> Result<DrawbridgeToken, Error> {
        drawbridge::get_token(
            &self.id,
            &self.attestation_token,
            self.enclave_type,
        )
    }
}
