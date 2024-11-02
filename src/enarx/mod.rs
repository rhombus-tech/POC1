pub struct EnarxConfig {
    pub attestation_endpoint: String,
    pub keep_config: KeepConfig,
}

pub struct KeepConfig {
    pub backend: String,
    pub attestation_required: bool,
    pub debug_mode: bool,
}

impl EnarxConfig {
    pub fn new() -> Self {
        Self {
            attestation_endpoint: "https://drawbridge.example.com".to_string(),
            keep_config: KeepConfig {
                backend: "auto".to_string(),
                attestation_required: true,
                debug_mode: false,
            },
        }
    }
}
