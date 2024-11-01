use super::common::*;
use crate::{types::*, state::*};

mod executor_registration {
    use super::*;

    #[test]
    fn test_sgx_executor_registration() {
        let mut context = setup();
        let sgx_executor = Address::from([3u8; 32]);

        context.set_caller(sgx_executor);
        register_executor(
            &mut context,
            EnclaveType::IntelSGX,
            SGX_OPERATOR.to_string(),
            vec![0u8; 32],
            vec![0u8; 64],
        );

        // Verify registration
        let executor_pool = context.get(ExecutorPool()).unwrap().unwrap();
        assert_eq!(executor_pool.sgx_executor, Some(sgx_executor));
        assert_eq!(get_current_phase(&mut context), Phase::Creation);
    }

    #[test]
    fn test_sev_executor_registration() {
        let mut context = setup();
        let sev_executor = Address::from([4u8; 32]);

        context.set_caller(sev_executor);
        register_executor(
            &mut context,
            EnclaveType::AMDSEV,
            SEV_OPERATOR.to_string(),
            vec![0u8; 32],
            vec![0u8; 64],
        );

        // Verify registration
        let executor_pool = context.get(ExecutorPool()).unwrap().unwrap();
        assert_eq!(executor_pool.sev_executor, Some(sev_executor));
        assert_eq!(get_current_phase(&mut context), Phase::Creation);
    }

    #[test]
    fn test_complete_executor_registration() {
        let mut context = setup();
        let sgx_executor = Address::from([3u8; 32]);
        let sev_executor = Address::from([4u8; 32]);

        // Register SGX executor
        context.set_caller(sgx_executor);
        register_executor(
            &mut context,
            EnclaveType::IntelSGX,
            SGX_OPERATOR.to_string(),
            vec![0u8; 32],
            vec![0u8; 64],
        );

        // Register SEV executor
        context.set_caller(sev_executor);
        register_executor(
            &mut context,
            EnclaveType::AMDSEV,
            SEV_OPERATOR.to_string(),
            vec![0u8; 32],
            vec![0u8; 64],
        );

        // Verify both registrations and phase transition
        let executor_pool = context.get(ExecutorPool()).unwrap().unwrap();
        assert_eq!(executor_pool.sgx_executor, Some(sgx_executor));
        assert_eq!(executor_pool.sev_executor, Some(sev_executor));
        assert_eq!(get_current_phase(&mut context), Phase::Executing);
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
            vec![0u8; 32],
            vec![0u8; 64],
        );

        // Attempt to register second SGX executor
        context.set_caller(sgx_executor2);
        register_executor(
            &mut context,
            EnclaveType::IntelSGX,
            SGX_OPERATOR.to_string(),
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
            vec![0u8; 32],
            vec![0u8; 64],
        );

        // Attempt to register second SEV executor
        context.set_caller(sev_executor2);
        register_executor(
            &mut context,
            EnclaveType::AMDSEV,
            SEV_OPERATOR.to_string(),
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

        let timestamp = context.get(HeartbeatTimestamp(sgx_executor)).unwrap().unwrap();
        assert!(timestamp > 0);

        let executor_pool = context.get(ExecutorPool()).unwrap().unwrap();
        assert_eq!(executor_pool.execution_count, 1);
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

        let executor_pool = context.get(ExecutorPool()).unwrap().unwrap();
        assert_eq!(executor_pool.execution_count, 6);
    }
}

mod executor_verification {
    use super::*;

    #[test]
    fn test_executor_attestation() {
        let mut context = setup();
        let sgx_executor = Address::from([3u8; 32]);

        context.set_caller(sgx_executor);
        register_executor(
            &mut context,
            EnclaveType::IntelSGX,
            SGX_OPERATOR.to_string(),
            vec![1u8; 32], // Unique attestation data
            vec![2u8; 64], // Unique signature
        );

        let attestation_status = context.get(AttestationStatus(sgx_executor)).unwrap().unwrap();
        assert!(attestation_status);
    }

    #[test]
    fn test_executor_type_verification() {
        let mut context = setup();
        let (sgx_executor, sev_executor, _) = setup_system(&mut context);

        // Verify SGX executor type
        let sgx_type = context.get(EnclaveType(sgx_executor)).unwrap().unwrap();
        assert_eq!(sgx_type, EnclaveType::IntelSGX);

        // Verify SEV executor type
        let sev_type = context.get(EnclaveType(sev_executor)).unwrap().unwrap();
        assert_eq!(sev_type, EnclaveType::AMDSEV);
    }
}

mod executor_phase_transitions {
    use super::*;

    #[test]
    fn test_phase_transitions() {
        let mut context = setup();
        
        // Initial phase
        assert_eq!(get_current_phase(&mut context), Phase::Creation);

        // Register SGX executor
        let sgx_executor = Address::from([3u8; 32]);
        context.set_caller(sgx_executor);
        register_executor(
            &mut context,
            EnclaveType::IntelSGX,
            SGX_OPERATOR.to_string(),
            vec![0u8; 32],
            vec![0u8; 64],
        );
        assert_eq!(get_current_phase(&mut context), Phase::Creation);

        // Register SEV executor
        let sev_executor = Address::from([4u8; 32]);
        context.set_caller(sev_executor);
        register_executor(
            &mut context,
            EnclaveType::AMDSEV,
            SEV_OPERATOR.to_string(),
            vec![0u8; 32],
            vec![0u8; 64],
        );
        assert_eq!(get_current_phase(&mut context), Phase::Executing);
    }
}
