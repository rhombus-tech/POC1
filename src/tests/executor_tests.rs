use super::common::*;
use crate::{types::*, state::*};

mod executor_registration {
    use super::*;

    #[test]
    fn test_sgx_executor_registration() {
        let mut context = setup();
        let sgx_executor = Address::from([3u8; 32]);

        // Include Enarx Keep data
        let keep_id = "sgx-keep-123".to_string();

        context.set_caller(sgx_executor);
        register_executor(
            &mut context,
            EnclaveType::IntelSGX,
            SGX_OPERATOR.to_string(),
            keep_id.clone(),
            vec![0u8; 32], // attestation report
            vec![0u8; 64], // drawbridge token
        );

        // Original verifications
        let executor_pool = context.get(ExecutorPool()).unwrap().unwrap();
        assert_eq!(executor_pool.sgx_executor, Some(sgx_executor));
        assert_eq!(get_current_phase(&mut context), Phase::Creation);

        // New Enarx-specific verifications
        let stored_keep_id = context.get(KeepId(sgx_executor)).unwrap().unwrap();
        assert_eq!(stored_keep_id, keep_id);
        assert!(context.get(KeepStatus(sgx_executor)).unwrap().unwrap());
    }

    #[test]
    fn test_sev_executor_registration() {
        let mut context = setup();
        let sev_executor = Address::from([4u8; 32]);

        // Include Enarx Keep data
        let keep_id = "sev-keep-456".to_string();

        context.set_caller(sev_executor);
        register_executor(
            &mut context,
            EnclaveType::AMDSEV,
            SEV_OPERATOR.to_string(),
            keep_id.clone(),
            vec![0u8; 32],
            vec![0u8; 64],
        );

        // Original verifications
        let executor_pool = context.get(ExecutorPool()).unwrap().unwrap();
        assert_eq!(executor_pool.sev_executor, Some(sev_executor));
        assert_eq!(get_current_phase(&mut context), Phase::Creation);

        // New Enarx-specific verifications
        let stored_keep_id = context.get(KeepId(sev_executor)).unwrap().unwrap();
        assert_eq!(stored_keep_id, keep_id);
        assert!(context.get(KeepStatus(sev_executor)).unwrap().unwrap());
    }

    #[test]
    fn test_complete_executor_registration() {
        let mut context = setup();
        let sgx_executor = Address::from([3u8; 32]);
        let sev_executor = Address::from([4u8; 32]);

        // Register SGX executor with Enarx Keep
        context.set_caller(sgx_executor);
        register_executor(
            &mut context,
            EnclaveType::IntelSGX,
            SGX_OPERATOR.to_string(),
            "sgx-keep-123".to_string(),
            vec![0u8; 32],
            vec![0u8; 64],
        );

        // Register SEV executor with Enarx Keep
        context.set_caller(sev_executor);
        register_executor(
            &mut context,
            EnclaveType::AMDSEV,
            SEV_OPERATOR.to_string(),
            "sev-keep-456".to_string(),
            vec![0u8; 32],
            vec![0u8; 64],
        );

        // Original verifications
        let executor_pool = context.get(ExecutorPool()).unwrap().unwrap();
        assert_eq!(executor_pool.sgx_executor, Some(sgx_executor));
        assert_eq!(executor_pool.sev_executor, Some(sev_executor));
        assert_eq!(get_current_phase(&mut context), Phase::Executing);

        // Verify both Keeps are active
        assert!(context.get(KeepStatus(sgx_executor)).unwrap().unwrap());
        assert!(context.get(KeepStatus(sev_executor)).unwrap().unwrap());
    }

    #[test]
    #[should_panic(expected = "SGX executor slot already filled")]
    fn test_duplicate_sgx_registration() {
        let mut context = setup();
        let sgx_executor1 = Address::from([3u8; 32]);
        let sgx_executor2 = Address::from([4u8; 32]);

        // Register first SGX executor
        context.set_caller(sgx_executor1);
        register_executor(
            &mut context,
            EnclaveType::IntelSGX,
            SGX_OPERATOR.to_string(),
            "sgx-keep-123".to_string(),
            vec![0u8; 32],
            vec![0u8; 64],
        );

        // Attempt to register second SGX executor
        context.set_caller(sgx_executor2);
        register_executor(
            &mut context,
            EnclaveType::IntelSGX,
            SGX_OPERATOR.to_string(),
            "sgx-keep-456".to_string(),
            vec![0u8; 32],
            vec![0u8; 64],
        );
    }

    #[test]
    #[should_panic(expected = "SEV executor slot already filled")]
    fn test_duplicate_sev_registration() {
        let mut context = setup();
        let sev_executor1 = Address::from([3u8; 32]);
        let sev_executor2 = Address::from([4u8; 32]);

        // Register first SEV executor
        context.set_caller(sev_executor1);
        register_executor(
            &mut context,
            EnclaveType::AMDSEV,
            SEV_OPERATOR.to_string(),
            "sev-keep-123".to_string(),
            vec![0u8; 32],
            vec![0u8; 64],
        );

        // Attempt to register second SEV executor
        context.set_caller(sev_executor2);
        register_executor(
            &mut context,
            EnclaveType::AMDSEV,
            SEV_OPERATOR.to_string(),
            "sev-keep-456".to_string(),
            vec![0u8; 32],
            vec![0u8; 64],
        );
    }
}

mod executor_operations {
    use super::*;

    #[test]
    fn test_executor_heartbeat() {
        let mut context = setup();
        let (sgx_executor, _, _) = setup_system(&mut context);

        context.set_caller(sgx_executor);
        submit_heartbeat(&mut context);

        // Original verifications
        let timestamp = context.get(HeartbeatTimestamp(sgx_executor)).unwrap().unwrap();
        assert!(timestamp > 0);

        let executor_pool = context.get(ExecutorPool()).unwrap().unwrap();
        assert_eq!(executor_pool.execution_count, 1);

        // New Enarx-specific verifications
        let keep_status = context.get(KeepStatus(sgx_executor)).unwrap().unwrap();
        assert!(keep_status, "Keep should remain active after heartbeat");
    }

    #[test]
    fn test_executor_state_updates() {
        let mut context = setup();
        let (sgx_executor, sev_executor, _) = setup_system(&mut context);

        // Submit multiple heartbeats
        for executor in [sgx_executor, sev_executor].iter() {
            context.set_caller(*executor);
            for _ in 0..3 {
                submit_heartbeat(&mut context);
            }
        }

        // Original verifications
        let executor_pool = context.get(ExecutorPool()).unwrap().unwrap();
        assert_eq!(executor_pool.execution_count, 6);

        // Verify Enarx Keep states
        for executor in [sgx_executor, sev_executor].iter() {
            assert!(context.get(KeepStatus(*executor)).unwrap().unwrap());
            let last_attestation = context
                .get(LastAttestationTime(*executor))
                .unwrap()
                .unwrap();
            assert!(last_attestation > 0);
        }
    }

    #[test]
    fn test_keep_attestation_renewal() {
        let mut context = setup();
        let (sgx_executor, _, _) = setup_system(&mut context);

        // Initial attestation time
        let initial_attestation = context
            .get(LastAttestationTime(sgx_executor))
            .unwrap()
            .unwrap();

        // Simulate time passing
        context.set_timestamp(initial_attestation + 1000);

        // Submit new attestation
        context.set_caller(sgx_executor);
        renew_attestation(
            &mut context,
            vec![1u8; 32], // new attestation report
            vec![2u8; 64], // new drawbridge token
        );

        // Verify attestation was updated
        let new_attestation = context
            .get(LastAttestationTime(sgx_executor))
            .unwrap()
            .unwrap();
        assert!(new_attestation > initial_attestation);
    }

    #[test]
    fn test_executor_keep_operations() {
        let mut context = setup();
        let (sgx_executor, sev_executor, _) = setup_system(&mut context);

        // Test Keep status checks
        for executor in [sgx_executor, sev_executor].iter() {
            context.set_caller(*executor);
            
            // Verify initial Keep state
            assert!(context.get(KeepStatus(*executor)).unwrap().unwrap());
            
            // Submit heartbeat with Keep status
            submit_heartbeat_with_keep_status(&mut context, true);
            
            // Verify Keep remains active
            assert!(context.get(KeepStatus(*executor)).unwrap().unwrap());
        }
    }

    #[test]
    fn test_keep_measurement_updates() {
        let mut context = setup();
        let (sgx_executor, _, _) = setup_system(&mut context);

        context.set_caller(sgx_executor);

        // Submit Keep measurement
        let measurement = vec![3u8; 32]; // Mock measurement
        update_keep_measurement(&mut context, measurement.clone());

        // Verify measurement was stored
        let stored_measurement = context
            .get(KeepMeasurement(sgx_executor))
            .unwrap()
            .unwrap();
        assert_eq!(stored_measurement, measurement);
    }

    #[test]
    #[should_panic(expected = "keep not active")]
    fn test_operation_with_inactive_keep() {
        let mut context = setup();
        let (sgx_executor, _, _) = setup_system(&mut context);

        // Deactivate Keep
        context.store_by_key(KeepStatus(sgx_executor), false)
            .expect("failed to update keep status");

        // Attempt operation with inactive Keep
        context.set_caller(sgx_executor);
        submit_heartbeat(&mut context);
    }

    #[test]
    fn test_keep_status_transitions() {
        let mut context = setup();
        let (sgx_executor, _, _) = setup_system(&mut context);

        context.set_caller(sgx_executor);

        // Test Keep pause
        pause_keep(&mut context);
        assert!(!context.get(KeepStatus(sgx_executor)).unwrap().unwrap());

        // Test Keep resume
        resume_keep(&mut context);
        assert!(context.get(KeepStatus(sgx_executor)).unwrap().unwrap());
    }

    #[test]
    fn test_concurrent_keep_operations() {
        let mut context = setup();
        let (sgx_executor, sev_executor, _) = setup_system(&mut context);

        // Simulate concurrent operations from both Keeps
        for executor in [sgx_executor, sev_executor].iter() {
            context.set_caller(*executor);
            
            // Submit heartbeat
            submit_heartbeat(&mut context);
            
            // Update measurement
            update_keep_measurement(&mut context, vec![4u8; 32]);
            
            // Renew attestation
            renew_attestation(
                &mut context,
                vec![5u8; 32],
                vec![6u8; 64],
            );
        }

        // Verify all operations were processed
        for executor in [sgx_executor, sev_executor].iter() {
            assert!(context.get(KeepStatus(*executor)).unwrap().unwrap());
            assert!(context.get(LastAttestationTime(*executor)).unwrap().unwrap() > 0);
            assert!(context.get(KeepMeasurement(*executor)).unwrap().is_some());
        }
    }
}

// Helper functions for Enarx operations
fn submit_heartbeat_with_keep_status(context: &mut Context, keep_active: bool) {
    let caller = context.actor();
    submit_heartbeat(context);
    context.store_by_key(KeepStatus(caller), keep_active)
        .expect("failed to update keep status");
}

fn update_keep_measurement(context: &mut Context, measurement: Vec<u8>) {
    let caller = context.actor();
    assert!(
        context.get(KeepStatus(caller)).unwrap().unwrap(),
        "keep not active"
    );
    context.store_by_key(KeepMeasurement(caller), measurement)
        .expect("failed to update measurement");
}

fn pause_keep(context: &mut Context) {
    let caller = context.actor();
    context.store_by_key(KeepStatus(caller), false)
        .expect("failed to pause keep");
}

fn resume_keep(context: &mut Context) {
    let caller = context.actor();
    context.store_by_key(KeepStatus(caller), true)
        .expect("failed to resume keep");
}

fn renew_attestation(context: &mut Context, attestation_report: Vec<u8>, drawbridge_token: Vec<u8>) {
    let caller = context.actor();
    assert!(
        context.get(KeepStatus(caller)).unwrap().unwrap(),
        "keep not active"
    );
    context.store_by_key(LastAttestationTime(caller), context.timestamp())
        .expect("failed to update attestation time");
}
