use super::common::*;
use crate::{types::*, state::*};

mod watchdog_registration {
    use super::*;

    #[test]
    fn test_watchdog_registration() {
        let mut context = setup();
        let (sgx_executor, sev_executor, _) = setup_system(&mut context);

        let watchdog1 = Address::from([5u8; 32]);
        let watchdog2 = Address::from([6u8; 32]);
        let watchdog3 = Address::from([7u8; 32]);

        // Register multiple watchdogs of different types
        context.set_caller(watchdog1);
        register_watchdog(
            &mut context,
            EnclaveType::IntelSGX,
            vec![0u8; 32],
            vec![0u8; 64],
        );

        context.set_caller(watchdog2);
        register_watchdog(
            &mut context,
            EnclaveType::AMDSEV,
            vec![0u8; 32],
            vec![0u8; 64],
        );

        context.set_caller(watchdog3);
        register_watchdog(
            &mut context,
            EnclaveType::IntelSGX,
            vec![0u8; 32],
            vec![0u8; 64],
        );

        // Verify watchdog registrations
        let watchdog_pool = context.get(WatchdogPool()).unwrap().unwrap();
        assert_eq!(watchdog_pool.watchdogs.len(), 3);

        // Verify enclave type distribution
        let sgx_watchdogs = watchdog_pool.watchdogs
            .iter()
            .filter(|(_, t)| *t == EnclaveType::IntelSGX)
            .count();
        let sev_watchdogs = watchdog_pool.watchdogs
            .iter()
            .filter(|(_, t)| *t == EnclaveType::AMDSEV)
            .count();

        assert_eq!(sgx_watchdogs, 2);
        assert_eq!(sev_watchdogs, 1);
    }

    #[test]
    fn test_watchdog_registration_phases() {
        let mut context = setup();

        // Register in Creation phase
        let watchdog1 = Address::from([5u8; 32]);
        context.set_caller(watchdog1);
        register_watchdog(
            &mut context,
            EnclaveType::IntelSGX,
            vec![0u8; 32],
            vec![0u8; 64],
        );

        // Setup executors to transition to Executing phase
        let (sgx_executor, sev_executor, _) = setup_system(&mut context);

        // Register in Executing phase
        let watchdog2 = Address::from([6u8; 32]);
        context.set_caller(watchdog2);
        register_watchdog(
            &mut context,
            EnclaveType::AMDSEV,
            vec![0u8; 32],
            vec![0u8; 64],
        );

        let watchdog_pool = context.get(WatchdogPool()).unwrap().unwrap();
        assert_eq!(watchdog_pool.watchdogs.len(), 2);
    }

    #[test]
    #[should_panic(expected = "watchdog already registered")]
    fn test_duplicate_watchdog_registration() {
        let mut context = setup();
        let watchdog = Address::from([5u8; 32]);

        context.set_caller(watchdog);
        register_watchdog(
            &mut context,
            EnclaveType::IntelSGX,
            vec![0u8; 32],
            vec![0u8; 64],
        );

        register_watchdog(
            &mut context,
            EnclaveType::IntelSGX,
            vec![0u8; 32],
            vec![0u8; 64],
        );
    }
}

mod watchdog_operations {
    use super::*;

    #[test]
    fn test_watchdog_heartbeat() {
        let mut context = setup();
        let (_, _, watchdog) = setup_system(&mut context);

        context.set_caller(watchdog);
        submit_heartbeat(&mut context);

        let timestamp = context.get(HeartbeatTimestamp(watchdog)).unwrap().unwrap();
        assert!(timestamp > 0);
    }

    #[test]
    fn test_watchdog_attestation_verification() {
        let mut context = setup();
        let watchdog = Address::from([5u8; 32]);

        context.set_caller(watchdog);
        
        let attestation_report = vec![1u8; 32];
        let tee_signature = vec![2u8; 64];

        register_watchdog(
            &mut context,
            EnclaveType::IntelSGX,
            attestation_report.clone(),
            tee_signature.clone(),
        );

        let attestation_status = context.get(AttestationStatus(watchdog)).unwrap().unwrap();
        assert!(attestation_status);
    }
}

mod watchdog_executor_replacement {
    use super::*;

    fn setup_challenge_state(context: &mut TestContext, executor: Address, watchdog: Address) -> u128 {
        context.set_caller(watchdog);
        challenge_executor(
            context,
            executor,
            ChallengeType::Attestation,
            vec![0u8; 32],
        );

        context.get(ChallengeCount()).unwrap().unwrap()
    }

    #[test]
    fn test_successful_executor_replacement() {
        let mut context = setup();
        let (sgx_executor, _, watchdog) = setup_system(&mut context);

        // Create challenge
        let challenge_id = setup_challenge_state(&mut context, sgx_executor, watchdog);

        // Fail the challenge
        let mut challenge = context.get(Challenge(challenge_id)).unwrap().unwrap();
        challenge.status = ChallengeStatus::Failed;
        context
            .store_by_key(Challenge(challenge_id), challenge)
            .expect("failed to update challenge");

        // Replace executor
        context.set_caller(watchdog);
        replace_executor(
            &mut context,
            sgx_executor,
            vec![0u8; 32], // New attestation
            vec![0u8; 64], // New signature
        );

        // Verify replacement
        let executor_pool = context.get(ExecutorPool()).unwrap().unwrap();
        assert_eq!(executor_pool.sgx_executor, Some(watchdog));

        // Verify watchdog removed from pool
        let watchdog_pool = context.get(WatchdogPool()).unwrap().unwrap();
        assert!(!watchdog_pool.watchdogs.iter().any(|(addr, _)| *addr == watchdog));
    }

    #[test]
    #[should_panic(expected = "enclave type mismatch")]
    fn test_invalid_replacement_type() {
        let mut context = setup();
        let (sgx_executor, _, watchdog) = setup_system(&mut context);

        // Register additional SEV watchdog
        let sev_watchdog = Address::from([6u8; 32]);
        context.set_caller(sev_watchdog);
        register_watchdog(
            &mut context,
            EnclaveType::AMDSEV,
            vec![0u8; 32],
            vec![0u8; 64],
        );

        // Create challenge
        let challenge_id = setup_challenge_state(&mut context, sgx_executor, watchdog);

        // Attempt replacement with wrong enclave type
        context.set_caller(sev_watchdog);
        replace_executor(
            &mut context,
            sgx_executor,
            vec![0u8; 32],
            vec![0u8; 64],
        );
    }
}

mod watchdog_pool_management {
    use super::*;

    #[test]
    fn test_watchdog_pool_scaling() {
        let mut context = setup();
        let (_, _, _) = setup_system(&mut context);

        // Register multiple watchdogs
        let mut watchdogs = Vec::new();
        for i in 0..10 {
            let watchdog = Address::from([i as u8 + 10; 32]);
            context.set_caller(watchdog);
            register_watchdog(
                &mut context,
                if i % 2 == 0 { EnclaveType::IntelSGX } else { EnclaveType::AMDSEV },
                vec![0u8; 32],
                vec![0u8; 64],
            );
            watchdogs.push(watchdog);
        }

        let watchdog_pool = context.get(WatchdogPool()).unwrap().unwrap();
        assert_eq!(watchdog_pool.watchdogs.len(), 10);

        // Verify type distribution
        let sgx_count = watchdog_pool.watchdogs
            .iter()
            .filter(|(_, t)| *t == EnclaveType::IntelSGX)
            .count();
        let sev_count = watchdog_pool.watchdogs
            .iter()
            .filter(|(_, t)| *t == EnclaveType::AMDSEV)
            .count();

        assert_eq!(sgx_count, 5);
        assert_eq!(sev_count, 5);
    }

    #[test]
    fn test_watchdog_pool_state() {
        let mut context = setup();
        let (_, _, _) = setup_system(&mut context);

        // Register and perform operations with watchdogs
        let watchdogs: Vec<Address> = (0..5)
            .map(|i| Address::from([i as u8 + 10; 32]))
            .collect();

        for (i, &watchdog) in watchdogs.iter().enumerate() {
            context.set_caller(watchdog);
            register_watchdog(
                &mut context,
                if i % 2 == 0 { EnclaveType::IntelSGX } else { EnclaveType::AMDSEV },
                vec![0u8; 32],
                vec![0u8; 64],
            );

            // Submit heartbeat
            submit_heartbeat(&mut context);
        }

        let watchdog_pool = context.get(WatchdogPool()).unwrap().unwrap();
        
        // Verify all watchdogs are active
        for &watchdog in &watchdogs {
            let heartbeat = context.get(HeartbeatTimestamp(watchdog)).unwrap().unwrap();
            assert!(heartbeat > 0);
        }
    }
}
