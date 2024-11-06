use std::fmt;
use thiserror::Error;
use wasmlanche::Error as WasmlancheError;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Executor error: {0}")]
    ExecutorError(String),
    
    #[error("Keep error: {0}")]
    KeepError(String),
    
    #[error("Challenge error: {0}")]
    ChallengeError(String),
    
    #[error("Verification error: {0}")]
    VerificationError(String),

    #[error("Executor not found")]
    ExecutorNotFound,

    #[error("Executor not active")]
    ExecutorNotActive,

    #[error("Unhealthy keep")]
    UnhealthyKeep,

    #[error("Execution not found")]
    ExecutionNotFound,

    #[error("Invalid evidence")]
    InvalidEvidence,

    #[error("No available watchdog")]
    NoAvailableWatchdog,

    #[error("Invalid attestation")]
    InvalidAttestation,

    #[error("Invalid Drawbridge token")]
    InvalidDrawbridgeToken,

    #[error("State error: {0}")]
    StateError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Enarx error: {0}")]
    EnarxError(String),

    #[error(transparent)]
    WasmlancheError(#[from] WasmlancheError),
}

// Implementation for converting from other error types
impl From<enarx::Error> for Error {
    fn from(err: enarx::Error) -> Self {
        Error::EnarxError(err.to_string())
    }
}

// Helper methods for error creation
impl Error {
    pub fn executor_error<T: Into<String>>(msg: T) -> Self {
        Error::ExecutorError(msg.into())
    }

    pub fn keep_error<T: Into<String>>(msg: T) -> Self {
        Error::KeepError(msg.into())
    }

    pub fn challenge_error<T: Into<String>>(msg: T) -> Self {
        Error::ChallengeError(msg.into())
    }

    pub fn verification_error<T: Into<String>>(msg: T) -> Self {
        Error::VerificationError(msg.into())
    }
}

// Result type alias for convenience
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_conversion() {
        let err = Error::executor_error("test error");
        assert!(matches!(err, Error::ExecutorError(_)));
    }

    #[test]
    fn test_error_display() {
        let err = Error::executor_error("test error");
        assert_eq!(err.to_string(), "Executor error: test error");
    }

    #[test]
    fn test_error_from_enarx() {
        // Simulate an Enarx error
        let enarx_err = enarx::Error::from("enarx test error");
        let err: Error = enarx_err.into();
        assert!(matches!(err, Error::EnarxError(_)));
    }
}
