use super::common::*;
use crate::types::Phase;

#[test]
fn test_initialization() {
    let mut context = setup();
    
    assert_eq!(
        get_current_phase(&mut context),
        Phase::Creation,
        "should start in Creation phase"
    );

    // Verify operator initialization
    let sgx_op = context
        .get(OperatorData(SGX_OPERATOR.to_string()))
        .unwrap()
        .expect("SGX operator should exist");
    
    let sev_op = context
        .get(OperatorData(SEV_OPERATOR.to_string()))
        .unwrap()
        .expect("SEV operator should exist");

    assert!(sgx_op.initialized);
    assert!(sev_op.initialized);

    // Verify pool initialization
    let executor_pool = context.get(ExecutorPool()).unwrap().unwrap();
    assert!(executor_pool.sgx_executor.is_none());
    assert!(executor_pool.sev_executor.is_none());

    let watchdog_pool = context.get(WatchdogPool()).unwrap().unwrap();
    assert!(watchdog_pool.watchdogs.is_empty());
}

#[test]
#[should_panic(expected = "system already initialized")]
fn test_double_initialization() {
    let mut context = setup();
    init(
        &mut context,
        SGX_OPERATOR.to_string(),
        SEV_OPERATOR.to_string(),
        Address::from([1u8; 32]),
        Address::from([2u8; 32]),
    );
}

#[test]
fn test_token_contract_initialization() {
    let mut context = setup();
    setup_with_token_contract(&mut context);

    let token_address = context.get(TokenContract()).unwrap().unwrap();
    assert_ne!(token_address, Address::from([0u8; 32]));
}

#[test]
fn test_system_stats_initial() {
    let mut context = setup();
    let (contracts, challenges, last_update) = get_system_stats(&mut context);
    
    assert_eq!(contracts, 0);
    assert_eq!(challenges, 0);
    assert!(last_update > 0);
}
