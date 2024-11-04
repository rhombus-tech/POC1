pub mod keep;
pub mod attestation;
pub mod drawbridge;

use wasmlanche::{Context, Address};
use std::path::PathBuf;

pub use self::keep::Keep;
pub use self::attestation::{AttestationReport, AttestationResult};
pub use self::drawbridge::{DrawbridgeToken, VerificationResult};

#[derive(Debug, Clone)]
pub struct EnarxConfig {
    pub keep_binary: PathBuf,
    pub attestation_url: String,
    pub drawbridge_url: String,
    pub debug_mode: bool,
}
