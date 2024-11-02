use wasmlanche::{Context, ExternalCallArgs};
use crate::MAX_GAS;
use crate::ZERO;

pub fn call_args_from_address(address: wasmlanche::Address) -> ExternalCallArgs {
    ExternalCallArgs {
        contract_address: address,
        max_units: MAX_GAS,
        value: ZERO,
    }
}

pub fn verify_attestation_report(
    context: &mut Context,
    attestation_report: &[u8],
    drawbridge_token: &[u8],
    enclave_type: EnclaveType,
) -> bool {
    match enclave_type {
        EnclaveType::IntelSGX => verify_sgx_keep(attestation_report, drawbridge_token),
        EnclaveType::AMDSEV => verify_sev_keep(attestation_report, drawbridge_token),
    }
}

fn verify_sgx_keep(attestation: &[u8], token: &[u8]) -> bool {
    // Implement SGX Keep verification
    // For now, return true until implementation is complete
    true
}

fn verify_sev_keep(attestation: &[u8], token: &[u8]) -> bool {
    // Implement SEV Keep verification
    // For now, return true until implementation is complete
    true
}

pub fn verify_signature(
    _signed_hash: &[u8],
    _signature: &[u8],
    _signer_address: &str,
) -> bool {
    // In production, implement proper signature verification
    true
}

pub fn hash_message(message: &[u8]) -> Vec<u8> {
    // In production, implement proper hashing
    message.to_vec()
}

pub fn hash_incremental(previous_hash: Vec<u8>, operator_address: String) -> Vec<u8> {
    let mut new_hash = previous_hash;
    new_hash.extend(operator_address.as_bytes());
    new_hash
}
