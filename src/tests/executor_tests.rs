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

mod executor_verification {
    use super::*;

    #[test]
    fn test_executor_attestation() {
        let mut context = setup();
        let sgx_executor = Address::from([3u8; 32]);

        // Register with Enarx Keep data
        context.set_caller(sgx_executor);
        register_executor(
            &mut context,
            EnclaveType::IntelSGX,
            SGX_OPERATOR.to_string(),
            "sgx-keep-123".to_string(),
            vec![1u8; 32], // Unique attestation data
            vec![2u8; 64], // Unique Drawbridge token
        );

        // Original verification
        let attestation_status = context.get(AttestationStatus(sgx_executor)).unwrap().unwrap();
        assert!(attestation_status);

        // Enarx-specific verifications
        let keep_status = context.get(KeepStatus(sgx_executor)).unwrap().unwrap();
        assert!(keep_status);
        
        let last_attestation = context.get(LastAttestationTime(sgx_executor)).unwrap().unwrap();
        assert!(last_attestation > 0);
    }

    #[test]
    fn test_executor_type_verification() {
        let mut context = setup();
        let (sgx_executor, sev_executor, _) = setup_system(&mut context);

        // Verify SGX executor type and Keep
        let sgx_type = context.get(EnclaveType(sgx_executor)).unwrap().unwrap();
        assert_eq!(sgx_type, EnclaveType::IntelSGX);
        let sgx_keep_id = context.get(KeepId(sgx_executor)).unwrap().unwrap();
        assert!(sgx_keep_id.starts_with("sgx"));

        // Verify SEV executor type and Keep
        let sev_type = context.get(EnclaveType(sev_executor)).unwrap().unwrap();
        assert_eq!(sev_type, EnclaveType::AMDSEV);
        let sev_keep_id = context.get(KeepId(sev_executor)).unwrap().unwrap();
        assert!(sev_keep_id.starts_with("sev"));
    }

    #[test]
    fn test_attestation_renewal() {
        let mut context = setup();
        let (sgx_executor, _, _) = setup_system(&mut context);

        let initial_attestation = context.get(LastAttestationTime(sgx_executor)).unwrap().unwrap();

        // Simulate time passing
        context.set_timestamp(initial_attestation + 1000);

        // Submit new attestation
        context.set_caller(sgx_executor);
        let new_attestation = vec![3u8; 32];
        let new_token = vec![4u8; 64];
        
        renew_attestation(
            &mut context,
            new_attestation.clone(),
            new_token,
        );

        // Verify attestation update
        let updated_time = context.get(LastAttestationTime(sgx_executor)).unwrap().unwrap();
        assert!(updated_time > initial_attestation);
    }

    #[test]
    fn test_keep_measurement_verification() {
        let mut context = setup();
        let (sgx_executor, _, _) = setup_system(&mut context);

        context.set_caller(sgx_executor);
        
        // Submit initial measurement
        let initial_measurement = vec![5u8; 32];
        update_keep_measurement(&mut context, initial_measurement.clone());

        // Verify measurement
        let stored_measurement = context.get(KeepMeasurement(sgx_executor)).unwrap().unwrap();
        assert_eq!(stored_measurement, initial_measurement);
    }

    #[test]
    #[should_panic(expected = "invalid attestation")]
    fn test_invalid_attestation() {
        let mut context = setup();
        let sgx_executor = Address::from([3u8; 32]);

        context.set_caller(sgx_executor);
        register_executor(
            &mut context,
            EnclaveType::IntelSGX,
            SGX_OPERATOR.to_string(),
            "invalid-keep".to_string(),
            vec![0u8; 32], // Invalid attestation
            vec![0u8; 64], // Invalid token
        );
    }

    #[test]
    fn test_drawbridge_token_verification() {
        let mut context = setup();
        let sgx_executor = Address::from([3u8; 32]);

        // Register with valid Drawbridge token
        context.set_caller(sgx_executor);
        let valid_token = vec![6u8; 64];
        
        register_executor(
            &mut context,
            EnclaveType::IntelSGX,
            SGX_OPERATOR.to_string(),
            "sgx-keep-123".to_string(),
            vec![1u8; 32],
            valid_token.clone(),
        );

        // Verify stored token
        let stored_token = context.get(DrawbridgeToken(sgx_executor)).unwrap().unwrap();
        assert_eq!(stored_token, valid_token);
    }

    #[test]
    fn test_keep_status_verification() {
        let mut context = setup();
        let (sgx_executor, _, _) = setup_system(&mut context);

        // Test Keep status transitions
        context.set_caller(sgx_executor);

        // Initial status should be active
        assert!(context.get(KeepStatus(sgx_executor)).unwrap().unwrap());

        // Pause Keep
        pause_keep(&mut context);
        assert!(!context.get(KeepStatus(sgx_executor)).unwrap().unwrap());

        // Resume Keep
        resume_keep(&mut context);
        assert!(context.get(KeepStatus(sgx_executor)).unwrap().unwrap());
    }

    #[test]
    fn test_attestation_expiration() {
        let mut context = setup();
        let (sgx_executor, _, _) = setup_system(&mut context);

        let initial_attestation = context.get(LastAttestationTime(sgx_executor)).unwrap().unwrap();

        // Simulate time passing beyond attestation validity
        context.set_timestamp(initial_attestation + ATTESTATION_VALIDITY_PERIOD + 1);

        // Verify attestation is expired
        context.set_caller(sgx_executor);
        assert!(!is_attestation_valid(&mut context, sgx_executor));
    }
}

// Constants for verification
const ATTESTATION_VALIDITY_PERIOD: u64 = 86400; // 24 hours in seconds

// Helper function for attestation validation
fn is_attestation_valid(context: &mut Context, executor: Address) -> bool {
    let last_attestation = context.get(LastAttestationTime(executor)).unwrap().unwrap();
    let current_time = context.timestamp();
    
    current_time - last_attestation <= ATTESTATION_VALIDITY_PERIOD
}

mod executor_phase_transitions {
    use super::*;

    #[test]
    fn test_phase_transitions() {
        let mut context = setup();
        
        // Initial phase
        assert_eq!(get_current_phase(&mut context), Phase::Creation);

        // Register SGX executor with Enarx Keep
        let sgx_executor = Address::from([3u8; 32]);
        context.set_caller(sgx_executor);
        register_executor(
            &mut context,
            EnclaveType::IntelSGX,
            SGX_OPERATOR.to_string(),
            "sgx-keep-123".to_string(),
            vec![0u8; 32],
            vec![0u8; 64],
        );
        assert_eq!(get_current_phase(&mut context), Phase::Creation);

        // Register SEV executor with Enarx Keep
        let sev_executor = Address::from([4u8; 32]);
        context.set_caller(sev_executor);
        register_executor(
            &mut context,
            EnclaveType::AMDSEV,
            SEV_OPERATOR.to_string(),
            "sev-keep-456".to_string(),
            vec![0u8; 32],
            vec![0u8; 64],
        );
        assert_eq!(get_current_phase(&mut context), Phase::Executing);

        // Verify Keep states after phase transition
        assert!(context.get(KeepStatus(sgx_executor)).unwrap().unwrap());
        assert!(context.get(KeepStatus(sev_executor)).unwrap().unwrap());
    }

    #[test]
    fn test_phase_transition_with_keep_verification() {
        let mut context = setup();
        let sgx_executor = Address::from([3u8; 32]);
        let sev_executor = Address::from([4u8; 32]);

        // Register both executors with initial attestations
        context.set_caller(sgx_executor);
        register_executor(
            &mut context,
            EnclaveType::IntelSGX,
            SGX_OPERATOR.to_string(),
            "sgx-keep-123".to_string(),
            vec![1u8; 32],
            vec![1u8; 64],
        );

        context.set_caller(sev_executor);
        register_executor(
            &mut context,
            EnclaveType::AMDSEV,
            SEV_OPERATOR.to_string(),
            "sev-keep-456".to_string(),
            vec![2u8; 32],
            vec![2u8; 64],
        );

        // Verify all states after transition
        assert_eq!(get_current_phase(&mut context), Phase::Executing);
        
        // Verify Keep states
        for executor in [sgx_executor, sev_executor].iter() {
            assert!(context.get(KeepStatus(*executor)).unwrap().unwrap());
            assert!(context.get(LastAttestationTime(*executor)).unwrap().unwrap() > 0);
            let keep_id = context.get(KeepId(*executor)).unwrap().unwrap();
            assert!(keep_id.contains("keep"));
        }
    }

    #[test]
    fn test_challenge_phase_transition() {
        let mut context = setup();
        let (sgx_executor, _, watchdogs) = setup_system(&mut context);

        // Initial phase should be Executing
        assert_eq!(get_current_phase(&mut context), Phase::Executing);

        // Create challenge
        context.set_caller(watchdogs[0]);
        challenge_executor(
            &mut context,
            sgx_executor,
            ChallengeType::Attestation,
            vec![0u8; 32],
        );

        // Verify transition to ChallengeExecutor phase
        assert_eq!(get_current_phase(&mut context), Phase::ChallengeExecutor);

        // Verify Keep remains active during challenge
        assert!(context.get(KeepStatus(sgx_executor)).unwrap().unwrap());
    }

    #[test]
    fn test_recovery_phase_transition() {
        let mut context = setup();
        let (sgx_executor, _, watchdogs) = setup_system(&mut context);

        // Create and fail challenge
        context.set_caller(watchdogs[0]);
        challenge_executor(
            &mut context,
            sgx_executor,
            ChallengeType::Attestation,
            vec![0u8; 32],
        );

        let challenge_id = context.get(ChallengeCount()).unwrap().unwrap();

        // Fail the challenge
        for watchdog in watchdogs.iter() {
            context.set_caller(*watchdog);
            verify_challenge_response(
                &mut context,
                challenge_id,
                false,
                vec![0u8; 32],
            );
        }

        // Replace executor
        context.set_caller(watchdogs[0]);
        replace_executor(
            &mut context,
            sgx_executor,
            vec![0u8; 32],
            vec![0u8; 64],
        );

        // Verify phase returned to Executing
        assert_eq!(get_current_phase(&mut context), Phase::Executing);

        // Verify new Keep is active and old one is inactive
        assert!(!context.get(KeepStatus(sgx_executor)).unwrap().unwrap());
        assert!(context.get(KeepStatus(watchdogs[0])).unwrap().unwrap());
    }

    #[test]
    fn test_phase_transitions_with_attestation_renewal() {
        let mut context = setup();
        let (sgx_executor, sev_executor, _) = setup_system(&mut context);

        // Simulate attestation expiration
        context.set_timestamp(context.timestamp() + ATTESTATION_VALIDITY_PERIOD + 1);

        // Renew attestations
        for executor in [sgx_executor, sev_executor].iter() {
            context.set_caller(*executor);
            renew_attestation(
                &mut context,
                vec![3u8; 32],
                vec![3u8; 64],
            );
        }

        // Verify system remains in Executing phase
        assert_eq!(get_current_phase(&mut context), Phase::Executing);

        // Verify renewed attestations
        for executor in [sgx_executor, sev_executor].iter() {
            assert!(is_attestation_valid(&mut context, *executor));
        }
    }

    #[test]
    #[should_panic(expected = "invalid phase transition")]
    fn test_invalid_phase_transition() {
        let mut context = setup();
        
        // Attempt to transition directly to Executing
        context.store_by_key(CurrentPhase(), Phase::Executing)
            .expect("failed to update phase");

        // This should panic as we haven't properly registered executors
        verify_phase_transition(&mut context);
    }

    #[test]
    fn test_keep_synchronization_during_transition() {
        let mut context = setup();
        let (sgx_executor, sev_executor, _) = setup_system(&mut context);

        // Verify Keep states are synchronized after transition
        let sgx_last_attestation = context
            .get(LastAttestationTime(sgx_executor))
            .unwrap()
            .unwrap();
        let sev_last_attestation = context
            .get(LastAttestationTime(sev_executor))
            .unwrap()
            .unwrap();

        // Attestation times should be close (within same block)
        assert!((sgx_last_attestation as i64 - sev_last_attestation as i64).abs() < 10);
    }
}

// Helper function for phase transition verification
fn verify_phase_transition(context: &mut Context) {
    let current_phase = get_current_phase(context);
    let executor_pool = context.get(ExecutorPool()).unwrap().unwrap();

    match current_phase {
        Phase::Executing => {
            assert!(executor_pool.sgx_executor.is_some(), "missing SGX executor");
            assert!(executor_pool.sev_executor.is_some(), "missing SEV executor");
        },
        Phase::Creation => {
            // Creation phase allows partial registration
        },
        _ => panic!("invalid phase transition"),
    }
}
