use enarx_keep_api::{Attestation, Keep};
use enarx_attestation::Evidence;
use std::time::{SystemTime, Duration};

#[derive(Debug, Clone)]
pub struct DrawbridgeToken {
    pub token: Vec<u8>,
    pub expiration: SystemTime,
    pub attestation: Attestation,
    pub evidence: Evidence,
    pub keep_id: String,
}

pub struct DrawbridgeClient {
    keep: Keep,
    last_token: Option<DrawbridgeToken>,
    token_refresh_interval: Duration,
}

impl DrawbridgeClient {
    pub async fn new(keep: Keep) -> Result<Self, DrawbridgeError> {
        Ok(Self {
            keep,
            last_token: None,
            token_refresh_interval: Duration::from_secs(3600), // 1 hour default
        })
    }

    pub async fn get_token(&mut self) -> Result<DrawbridgeToken, DrawbridgeError> {
        // Check if we need to refresh the token
        if let Some(token) = &self.last_token {
            if SystemTime::now() < token.expiration {
                return Ok(token.clone());
            }
        }

        // Get fresh attestation from Keep
        let attestation = self.keep.get_attestation().await?;
        
        // Get evidence from Keep
        let evidence = self.keep.get_evidence().await?;

        // Create token request
        let token_request = DrawbridgeTokenRequest {
            attestation: attestation.clone(),
            evidence: evidence.clone(),
            keep_id: self.keep.id().to_string(),
        };

        // Get new token
        let token = self.request_new_token(token_request).await?;

        // Store token
        self.last_token = Some(token.clone());

        Ok(token)
    }

    async fn request_new_token(
        &self,
        request: DrawbridgeTokenRequest,
    ) -> Result<DrawbridgeToken, DrawbridgeError> {
        // Generate proof from Keep's attestation and evidence
        let proof = request.generate_proof()?;

        // Create token with expiration
        let token = DrawbridgeToken {
            token: proof.token,
            expiration: SystemTime::now() + self.token_refresh_interval,
            attestation: request.attestation,
            evidence: request.evidence,
            keep_id: request.keep_id,
        };

        Ok(token)
    }

    pub async fn verify_token(&self, token: &DrawbridgeToken) -> Result<bool, DrawbridgeError> {
        // Verify token hasn't expired
        if SystemTime::now() > token.expiration {
            return Ok(false);
        }

        // Verify Keep attestation
        let attestation_valid = self.keep
            .verify_attestation(&token.attestation)
            .await?;

        // Verify evidence
        let evidence_valid = self.keep
            .verify_evidence(&token.evidence)
            .await?;

        Ok(attestation_valid && evidence_valid)
    }
}

#[derive(Debug)]
struct DrawbridgeTokenRequest {
    attestation: Attestation,
    evidence: Evidence,
    keep_id: String,
}

impl DrawbridgeTokenRequest {
    fn generate_proof(&self) -> Result<DrawbridgeProof, DrawbridgeError> {
        // Combine attestation and evidence into proof
        let mut proof_data = Vec::new();
        
        // Add attestation
        proof_data.extend_from_slice(&self.attestation.as_bytes());
        
        // Add evidence
        proof_data.extend_from_slice(&self.evidence.as_bytes());
        
        // Add Keep ID
        proof_data.extend_from_slice(self.keep_id.as_bytes());

        Ok(DrawbridgeProof {
            token: proof_data,
        })
    }
}

#[derive(Debug)]
struct DrawbridgeProof {
    token: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum DrawbridgeError {
    #[error("Keep error: {0}")]
    KeepError(#[from] enarx_keep_api::Error),
    
    #[error("Attestation error: {0}")]
    AttestationError(String),
    
    #[error("Evidence error: {0}")]
    EvidenceError(String),
    
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Invalid token")]
    InvalidToken,
}

// Integration with Keep
impl Keep {
    pub async fn get_drawbridge_token(&mut self) -> Result<DrawbridgeToken, DrawbridgeError> {
        let mut client = DrawbridgeClient::new(self.clone()).await?;
        client.get_token().await
    }

    pub async fn verify_drawbridge_token(
        &self,
        token: &DrawbridgeToken,
    ) -> Result<bool, DrawbridgeError> {
        let client = DrawbridgeClient::new(self.clone()).await?;
        client.verify_token(token).await
    }
}

// Usage in executor
impl Executor {
    async fn verify_token_before_execution(
        &self,
        token: &DrawbridgeToken,
    ) -> Result<(), Error> {
        // Verify Drawbridge token before execution
        if !self.keep.verify_drawbridge_token(token).await? {
            return Err(Error::InvalidDrawbridgeToken);
        }
        Ok(())
    }

    pub async fn execute_with_verification(
        &self,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, Error> {
        // Get current token
        let token = self.keep.get_drawbridge_token().await?;

        // Verify token
        self.verify_token_before_execution(&token).await?;

        // Execute if token is valid
        self.keep.execute(payload).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_lifecycle() {
        // Create Keep
        let keep = Keep::new(&KeepConfig::default(), EnclaveType::IntelSGX)
            .await
            .unwrap();

        // Get initial token
        let token = keep.get_drawbridge_token().await.unwrap();

        // Verify token
        assert!(keep.verify_drawbridge_token(&token).await.unwrap());

        // Verify expired token fails
        let mut expired_token = token.clone();
        expired_token.expiration = SystemTime::now() - Duration::from_secs(1);
        assert!(!keep.verify_drawbridge_token(&expired_token).await.unwrap());
    }

    #[tokio::test]
    async fn test_token_refresh() {
        let keep = Keep::new(&KeepConfig::default(), EnclaveType::IntelSGX)
            .await
            .unwrap();

        let mut client = DrawbridgeClient::new(keep).await.unwrap();
        
        // Get initial token
        let token1 = client.get_token().await.unwrap();
        
        // Force refresh
        client.token_refresh_interval = Duration::from_secs(0);
        let token2 = client.get_token().await.unwrap();

        // Tokens should be different
        assert_ne!(token1.token, token2.token);
    }
}
