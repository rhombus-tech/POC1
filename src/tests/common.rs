use wasmlanche::testing::{setup_test, TestContext};
use crate::{
    types::*,
    state::*,
    core::*,
    challenge::*,
    external::*,
};

pub const SGX_OPERATOR: &str = "sgx_operator_address";
pub const SEV_OPERATOR: &str = "sev_operator_address";

pub fn setup() -> TestContext {
    let mut context = setup_test();
    init(
        &mut context,
        SGX_OPERATOR.to_string(),
        SEV_OPERATOR.to_string(),
        Address::from([1u8; 32]), // Mock token contract
        Address::from([2u8; 32]), // Mock governance contract
    );
    context
}

pub fn setup_with_token_contract(context: &mut TestContext) {
    init_token_contract(
        context,
        ContractId::from([0u8; 32]),
        1_000_000,
    );
}

pub fn setup_system(context: &mut TestContext) -> (Address, Address, Address) {
    let sgx_executor = Address::from([3u8; 32]);
    let sev_executor = Address::from([4u8; 32]);
    let watchdog = Address::from([5u8; 32]);

    // Register executors
    context.set_caller(sgx_executor);
    register_executor(
        context,
        EnclaveType::IntelSGX,
        SGX_OPERATOR.to_string(),
        vec![0u8; 32], // Mock attestation report
        vec![0u8; 64], // Mock signature
    );

    context.set_caller(sev_executor);
    register_executor(
        context,
        EnclaveType::AMDSEV,
        SEV_OPERATOR.to_string(),
        vec![0u8; 32],
        vec![0u8; 64],
    );

    // Register watchdog
    context.set_caller(watchdog);
    register_watchdog(
        context,
        EnclaveType::IntelSGX,
        vec![0u8; 32],
        vec![0u8; 64],
    );

    (sgx_executor, sev_executor, watchdog)
}

pub fn setup_full_system(context: &mut TestContext) -> (Address, Address, Vec<Address>) {
    let sgx_executor = Address::from([3u8; 32]);
    let sev_executor = Address::from([4u8; 32]);
    let mut watchdogs = Vec::new();

    // Register executors
    context.set_caller(sgx_executor);
    register_executor(
        context,
        EnclaveType::IntelSGX,
        SGX_OPERATOR.to_string(),
        vec![0u8; 32],
        vec![0u8; 64],
    );

    context.set_caller(sev_executor);
    register_executor(
        context,
        EnclaveType::AMDSEV,
        SEV_OPERATOR.to_string(),
        vec![0u8; 32],
        vec![0u8; 64],
    );

    // Register multiple watchdogs
    for i in 0..3 {
        let watchdog = Address::from([(i + 5) as u8; 32]);
        context.set_caller(watchdog);
        register_watchdog(
            context,
            if i % 2 == 0 { EnclaveType::IntelSGX } else { EnclaveType::AMDSEV },
            vec![0u8; 32],
            vec![0u8; 64],
        );
        watchdogs.push(watchdog);
    }

    (sgx_executor, sev_executor, watchdogs)
}
