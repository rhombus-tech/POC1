use crate::error::{Error, Result};
use enarx_attestation::{
    attester::{self, Attester},
    verifier::{self, Verifier},
    snp::AttestationReport as SnpReport,
    sgx::Quote as SgxQuote,
};
use enarx_keep_api;

#[derive(Debug, Clone)]
pub struct AttestationReport {
    pub keep_id: String,
    pub timestamp: u64,
    pub enclave_type: EnclaveType,
    pub measurement: Vec<u8>,
    // Enarx-specific fields
    pub keep_attestation: enarx_keep_api::Attestation,
    pub platform_ enarx_attestation::PlatformData,
}

#[derive(Debug, Clone)]
pub struct AttestationResult {
    pub valid: bool,
    pub timestamp: u64,
    pub report: AttestationReport,
}

pub fn verify_attestation(
    attestation_token: &[u8],
    measurement: &[u8],
    enclave_type: EnclaveType,
) -> Result<AttestationResult> {
    match enclave_type {
        EnclaveType::IntelSGX => verify_sgx_attestation(attestation_token, measurement),
        EnclaveType::AMDSEV => verify_sev_attestation(attestation_token, measurement),
    }
}

fn verify_sgx_attestation(token: &[u8], measurement: &[u8]) -> Result<AttestationResult> {
    // Get the Keep's attestation
    let keep_attestation = enarx_keep_api::get_attestation()
        .map_err(|e| Error::keep_error(format!("Failed to get attestation: {}", e)))?;

    // Use Enarx's SGX attester
    let attester = attester::sgx::Attester::new()
        .map_err(|e| Error::attestation_error(format!("Failed to create attester: {}", e)))?;
    let quote = attester.generate_quote(&keep_attestation)
        .map_err(|e| Error::attestation_error(format!("Failed to generate quote: {}", e)))?;

    // Use Enarx's SGX verifier
    let verifier = verifier::sgx::Verifier::new()
        .map_err(|e| Error::attestation_error(format!("Failed to create verifier: {}", e)))?;
    let verification = verifier.verify(&quote)
        .map_err(|e| Error::attestation_error(format!("Failed to verify quote: {}", e)))?;

    // Verify measurement matches Keep's measurement
    if verification.measurement != measurement {
        return Err(Error::attestation_error("Measurement mismatch"));
    }

    Ok(AttestationResult {
        valid: true,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::time_error(e))?
            .as_secs(),
        report: AttestationReport {
            keep_id: keep_attestation.keep_id.to_string(),
            timestamp: verification.timestamp,
            enclave_type: EnclaveType::IntelSGX,
            measurement: measurement.to_vec(),
            keep_attestation,
            platform_ verification.platform_data,
        },
    })
}

fn verify_sev_attestation(token: &[u8], measurement: &[u8]) -> Result<AttestationResult> {
    // Get the Keep's attestation
    let keep_attestation = enarx_keep_api::get_attestation()
        .map_err(|e| Error::keep_error(format!("Failed to get attestation: {}", e)))?;

    // Use Enarx's SEV attester
    let attester = attester::snp::Attester::new()
        .map_err(|e| Error::attestation_error(format!("Failed to create attester: {}", e)))?;
    let report = attester.generate_report(&keep_attestation)
        .map_err(|e| Error::attestation_error(format!("Failed to generate report: {}", e)))?;

    // Use Enarx's SEV verifier
    let verifier = verifier::snp::Verifier::new()
        .map_err(|e| Error::attestation_error(format!("Failed to create verifier: {}", e)))?;
    let verification = verifier.verify(&report)
        .map_err(|e| Error::attestation_error(format!("Failed to verify report: {}", e)))?;

    // Verify measurement matches Keep's measurement
    if verification.measurement != measurement {
        return Err(Error::attestation_error("Measurement mismatch"));
    }

    Ok(AttestationResult {
        valid: true,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::time_error(e))?
            .as_secs(),
        report: AttestationReport {
            keep_id: keep_attestation.keep_id.to_string(),
            timestamp: verification.timestamp,
            enclave_type: EnclaveType::AMDSEV,
            measurement: measurement.to_vec(),
            keep_attestation,
            platform_ verification.platform_data,
        },
    })
}

// Enarx Keep management
impl Keep {
    pub fn new(config: &EnarxConfig, enclave_type: EnclaveType) -> Result<Self> {
        // Launch Keep using Enarx's API
        let keep = enarx_keep_api::Keep::launch(&config.keep_binary)
            .map_err(|e| Error::keep_error(format!("Failed to launch keep: {}", e)))?;

        // Get initial attestation
        let attestation = keep.get_attestation()
            .map_err(|e| Error::attestation_error(format!("Failed to get initial attestation: {}", e)))?;

        Ok(Self {
            id: keep.id().to_string(),
            enclave_type,
            keep,
            attestation,
            measurement: keep.get_measurement()
                .map_err(|e| Error::keep_error(format!("Failed to get measurement: {}", e)))?,
        })
    }

    pub fn verify_attestation(&self) -> Result<AttestationResult> {
        match self.enclave_type {
            EnclaveType::IntelSGX => verify_sgx_attestation(
                &self.attestation.as_bytes(),
                &self.measurement,
            ),
            EnclaveType::AMDSEV => verify_sev_attestation(
                &self.attestation.as_bytes(),
                &self.measurement,
            ),
        }
    }

    pub fn execute(&self, payload: Vec<u8>) -> Result<Vec<u8>> {
        self.keep.execute(payload)
            .map_err(|e| Error::keep_error(format!("Execution failed: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sgx_attestation() -> Result<()> {
        // ... test implementation
        Ok(())
    }
}
