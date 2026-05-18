use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// JWT claims for bridge authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeClaims {
    /// Subject (agent ID or session ID).
    pub sub: String,
    /// Issuer.
    pub iss: String,
    /// Audience.
    pub aud: String,
    /// Expiration time (seconds since epoch).
    pub exp: u64,
    /// Issued at (seconds since epoch).
    pub iat: u64,
    /// Not before (seconds since epoch).
    pub nbf: u64,
    /// Session ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Environment ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_id: Option<String>,
    /// Agent ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Team name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_name: Option<String>,
    /// Permissions scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<Vec<String>>,
}

/// Token configuration.
#[derive(Debug, Clone)]
pub struct TokenConfig {
    /// Secret key for signing (HMAC-SHA256).
    pub secret: Vec<u8>,
    /// Token TTL.
    pub ttl: Duration,
    /// Issuer.
    pub issuer: String,
    /// Audience.
    pub audience: String,
}

impl TokenConfig {
    pub fn new(secret: &str, ttl: Duration) -> Self {
        Self {
            secret: secret.as_bytes().to_vec(),
            ttl,
            issuer: "claude-code-bridge".to_string(),
            audience: "claude-code-daemon".to_string(),
        }
    }
}

/// JWT token with claims and signature.
#[derive(Debug, Clone)]
pub struct JwtToken {
    pub header: String,
    pub payload: String,
    pub signature: String,
}

impl JwtToken {
    /// Encode to the standard JWT string format.
    pub fn encode(&self) -> String {
        format!("{}.{}.{}", self.header, self.payload, self.signature)
    }

    /// Parse from a JWT string.
    pub fn decode(token: &str) -> Result<Self, String> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err("Invalid JWT: expected 3 parts".to_string());
        }
        Ok(Self {
            header: parts[0].to_string(),
            payload: parts[1].to_string(),
            signature: parts[2].to_string(),
        })
    }
}

/// JWT utility functions for bridge authentication.
pub struct JwtUtils;

impl JwtUtils {
    /// Create a new JWT token with the given claims.
    pub fn create_token(claims: &BridgeClaims, config: &TokenConfig) -> Result<JwtToken, String> {
        // Header: {"alg": "HS256", "typ": "JWT"}
        let header = serde_json::json!({
            "alg": "HS256",
            "typ": "JWT"
        });
        let header_b64 = base64_encode(&serde_json::to_vec(&header).map_err(|e| e.to_string())?);

        // Payload: claims as JSON
        let payload_b64 = base64_encode(&serde_json::to_vec(claims).map_err(|e| e.to_string())?);

        // Signature: HMAC-SHA256(header.payload, secret)
        let signing_input = format!("{}.{}", header_b64, payload_b64);
        let signature = hmac_sha256(signing_input.as_bytes(), &config.secret);
        let signature_b64 = base64_encode(&signature);

        Ok(JwtToken {
            header: header_b64,
            payload: payload_b64,
            signature: signature_b64,
        })
    }

    /// Validate a JWT token: check signature and claims.
    pub fn validate_token(token: &str, config: &TokenConfig) -> Result<BridgeClaims, String> {
        let jwt = JwtToken::decode(token)?;

        // Verify signature
        let signing_input = format!("{}.{}", jwt.header, jwt.payload);
        let expected_signature = hmac_sha256(signing_input.as_bytes(), &config.secret);
        let expected_b64 = base64_encode(&expected_signature);

        if jwt.signature != expected_b64 {
            return Err("Invalid token signature".to_string());
        }

        // Decode payload
        let payload_bytes = base64_decode(&jwt.payload)?;
        let claims: BridgeClaims =
            serde_json::from_slice(&payload_bytes).map_err(|e| format!("Invalid claims: {}", e))?;

        // Check expiration
        let now = current_timestamp();
        if claims.exp < now {
            return Err("Token has expired".to_string());
        }

        // Check not-before
        if claims.nbf > now {
            return Err("Token is not yet valid".to_string());
        }

        // Check issuer
        if claims.iss != config.issuer {
            return Err(format!(
                "Invalid issuer: expected {}, got {}",
                config.issuer, claims.iss
            ));
        }

        // Check audience
        if claims.aud != config.audience {
            return Err(format!(
                "Invalid audience: expected {}, got {}",
                config.audience, claims.aud
            ));
        }

        debug!(sub = claims.sub, "Token validated successfully");
        Ok(claims)
    }

    /// Refresh an existing token with a new expiration.
    pub fn refresh_token(token: &str, config: &TokenConfig) -> Result<JwtToken, String> {
        let claims = Self::validate_token(token, config)?;

        // Create new claims with updated timestamps
        let now = current_timestamp();
        let new_claims = BridgeClaims {
            exp: now + config.ttl.as_secs(),
            iat: now,
            nbf: now,
            ..claims
        };

        Self::create_token(&new_claims, config)
    }

    /// Create a token for a specific session.
    pub fn create_session_token(
        session_id: &str,
        agent_id: &str,
        team_name: &str,
        config: &TokenConfig,
    ) -> Result<JwtToken, String> {
        let now = current_timestamp();
        let claims = BridgeClaims {
            sub: agent_id.to_string(),
            iss: config.issuer.clone(),
            aud: config.audience.clone(),
            exp: now + config.ttl.as_secs(),
            iat: now,
            nbf: now,
            session_id: Some(session_id.to_string()),
            environment_id: None,
            agent_id: Some(agent_id.to_string()),
            team_name: Some(team_name.to_string()),
            scope: Some(vec![
                "session:read".to_string(),
                "session:write".to_string(),
                "tools:execute".to_string(),
            ]),
        };

        Self::create_token(&claims, config)
    }

    /// Check if a token has a specific permission scope.
    pub fn has_scope(claims: &BridgeClaims, scope: &str) -> bool {
        claims
            .scope
            .as_ref()
            .map(|scopes| scopes.iter().any(|s| s == scope))
            .unwrap_or(false)
    }

    /// Check if a token is about to expire (within the given duration).
    pub fn is_expiring_soon(claims: &BridgeClaims, threshold: Duration) -> bool {
        let now = current_timestamp();
        let threshold_secs = threshold.as_secs();
        claims.exp.saturating_sub(now) < threshold_secs
    }

    /// Get remaining token lifetime.
    pub fn remaining_lifetime(claims: &BridgeClaims) -> Duration {
        let now = current_timestamp();
        let remaining = claims.exp.saturating_sub(now);
        Duration::from_secs(remaining)
    }
}

/// Get current timestamp in seconds since epoch.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Base64 URL-safe encoding (no padding).
fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// Base64 URL-safe decoding.
fn base64_decode(data: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(data)
        .map_err(|e| format!("Base64 decode error: {}", e))
}

/// HMAC-SHA256 implementation.
fn hmac_sha256(data: &[u8], key: &[u8]) -> Vec<u8> {
    use hmac::Hmac;
    use hmac::Mac;
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(key)
        .expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}
