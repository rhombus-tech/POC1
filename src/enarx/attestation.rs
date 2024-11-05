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

fn verify_sgx_attestation(token: &[u8], measurement: &[u8]) -> Result<AttestationResult, Error> {
    // Get the Keep's attestation
    let keep_attestation = enarx_keep_api::get_attestation()?;

    // Use Enarx's SGX attester
    let attester = attester::sgx::Attester::new()?;
    let quote = attester.generate_quote(&keep_attestation)?;

    // Use Enarx's SGX verifier
    let verifier = verifier::sgx::Verifier::new()?;
    let verification = verifier.verify(&quote)?;

    // Verify measurement matches Keep's measurement
    if verification.measurement != measurement {
        return Err(Error::AttestationFailed("Measurement mismatch".to_string()));
    }

    Ok(AttestationResult {
        valid: true,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
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

fn verify_sev_attestation(token: &[u8], measurement: &[u8]) -> Result<AttestationResult, Error> {
    // Get the Keep's attestation
    let keep_attestation = enarx_keep_api::get_attestation()?;

    // Use Enarx's SEV attester
    let attester = attester::snp::Attester::new()?;
    let report = attester.generate_report(&keep_attestation)?;

    // Use Enarx's SEV verifier
    let verifier = verifier::snp::Verifier::new()?;
    let verification = verifier.verify(&report)?;

    // Verify measurement matches Keep's measurement
    if verification.measurement != measurement {
        return Err(Error::AttestationFailed("Measurement mismatch".to_string()));
    }

    Ok(AttestationResult {
        valid: true,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
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
    pub fn new(config: &EnarxConfig, enclave_type: EnclaveType) -> Result<Self, Error> {
        // Launch Keep using Enarx's API
        let keep = enarx_keep_api::Keep::launch(&config.keep_binary)?;

        // Get initial attestation
        let attestation = keep.get_attestation()?;

        Ok(Self {
            id: keep.id().to_string(),
            enclave_type,
            keep: keep,
            attestation,
            measurement: keep.get_measurement()?,
        })
    }

    pub fn verify_attestation(&self) -> Result<AttestationResult, Error> {
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

    pub fn execute(&self, payload: Vec<u8>) -> Result<Vec<u8>, Error> {
        // Execute payload in Keep
        self.keep.execute(payload)
    }
}

#[derive(Debug)]
pub enum Error {
    KeepLaunchFailed(enarx_keep_api::Error),
    AttestationFailed(String),
    ExecutionFailed(String),
    AttesterError(attester::Error),
    VerifierError(verifier::Error),
}
