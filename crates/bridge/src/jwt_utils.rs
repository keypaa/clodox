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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> TokenConfig {
        TokenConfig::new("test-secret-key", Duration::from_secs(3600))
    }

    fn test_claims() -> BridgeClaims {
        let now = current_timestamp();
        BridgeClaims {
            sub: "test-agent".to_string(),
            iss: "claude-code-bridge".to_string(),
            aud: "claude-code-daemon".to_string(),
            exp: now + 3600,
            iat: now,
            nbf: now,
            session_id: Some("session-123".to_string()),
            environment_id: Some("env-456".to_string()),
            agent_id: Some("agent-789".to_string()),
            team_name: Some("test-team".to_string()),
            scope: Some(vec!["session:read".to_string(), "session:write".to_string()]),
        }
    }

    #[test]
    fn test_create_and_encode_token() {
        let config = test_config();
        let claims = test_claims();
        let token = JwtUtils::create_token(&claims, &config).unwrap();
        let encoded = token.encode();
        assert_eq!(encoded.split('.').count(), 3);
    }

    #[test]
    fn test_validate_token_success() {
        let config = test_config();
        let claims = test_claims();
        let token = JwtUtils::create_token(&claims, &config).unwrap();
        let decoded = JwtUtils::validate_token(&token.encode(), &config).unwrap();
        assert_eq!(decoded.sub, "test-agent");
        assert_eq!(decoded.session_id, Some("session-123".to_string()));
    }

    #[test]
    fn test_validate_token_wrong_secret() {
        let config = test_config();
        let claims = test_claims();
        let token = JwtUtils::create_token(&claims, &config).unwrap();
        let wrong_config = TokenConfig::new("wrong-secret", Duration::from_secs(3600));
        let result = JwtUtils::validate_token(&token.encode(), &wrong_config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid token signature"));
    }

    #[test]
    fn test_validate_token_expired() {
        let config = test_config();
        let now = current_timestamp();
        let mut claims = test_claims();
        claims.exp = now - 100;
        let token = JwtUtils::create_token(&claims, &config).unwrap();
        let result = JwtUtils::validate_token(&token.encode(), &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expired"));
    }

    #[test]
    fn test_validate_token_not_yet_valid() {
        let config = test_config();
        let now = current_timestamp();
        let mut claims = test_claims();
        claims.nbf = now + 3600;
        let token = JwtUtils::create_token(&claims, &config).unwrap();
        let result = JwtUtils::validate_token(&token.encode(), &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not yet valid"));
    }

    #[test]
    fn test_validate_token_wrong_issuer() {
        let config = test_config();
        let mut claims = test_claims();
        claims.iss = "wrong-issuer".to_string();
        let token = JwtUtils::create_token(&claims, &config).unwrap();
        let result = JwtUtils::validate_token(&token.encode(), &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid issuer"));
    }

    #[test]
    fn test_validate_token_wrong_audience() {
        let config = test_config();
        let mut claims = test_claims();
        claims.aud = "wrong-audience".to_string();
        let token = JwtUtils::create_token(&claims, &config).unwrap();
        let result = JwtUtils::validate_token(&token.encode(), &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid audience"));
    }

    #[test]
    fn test_validate_invalid_jwt_format() {
        let config = test_config();
        let result = JwtUtils::validate_token("not.a.valid.jwt.format", &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_refresh_token() {
        let config = test_config();
        let claims = test_claims();
        let token = JwtUtils::create_token(&claims, &config).unwrap();
        let refreshed = JwtUtils::refresh_token(&token.encode(), &config).unwrap();
        let decoded = JwtUtils::validate_token(&refreshed.encode(), &config).unwrap();
        assert_eq!(decoded.sub, "test-agent");
        assert!(decoded.iat >= current_timestamp());
    }

    #[test]
    fn test_refresh_expired_token() {
        let config = test_config();
        let now = current_timestamp();
        let mut claims = test_claims();
        claims.exp = now - 100;
        let token = JwtUtils::create_token(&claims, &config).unwrap();
        let result = JwtUtils::refresh_token(&token.encode(), &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_session_token() {
        let config = test_config();
        let token = JwtUtils::create_session_token("sess-1", "agent-1", "team-1", &config).unwrap();
        let decoded = JwtUtils::validate_token(&token.encode(), &config).unwrap();
        assert_eq!(decoded.session_id, Some("sess-1".to_string()));
        assert_eq!(decoded.agent_id, Some("agent-1".to_string()));
        assert_eq!(decoded.team_name, Some("team-1".to_string()));
        assert!(decoded.scope.is_some());
    }

    #[test]
    fn test_has_scope() {
        let claims = test_claims();
        assert!(JwtUtils::has_scope(&claims, "session:read"));
        assert!(JwtUtils::has_scope(&claims, "session:write"));
        assert!(!JwtUtils::has_scope(&claims, "admin"));
    }

    #[test]
    fn test_has_scope_no_scope_field() {
        let mut claims = test_claims();
        claims.scope = None;
        assert!(!JwtUtils::has_scope(&claims, "session:read"));
    }

    #[test]
    fn test_is_expiring_soon() {
        let claims = test_claims();
        assert!(!JwtUtils::is_expiring_soon(&claims, Duration::from_secs(60)));
    }

    #[test]
    fn test_is_expiring_soon_threshold() {
        let now = current_timestamp();
        let mut claims = test_claims();
        claims.exp = now + 30;
        assert!(JwtUtils::is_expiring_soon(&claims, Duration::from_secs(60)));
    }

    #[test]
    fn test_remaining_lifetime() {
        let now = current_timestamp();
        let mut claims = test_claims();
        claims.exp = now + 1800;
        let remaining = JwtUtils::remaining_lifetime(&claims);
        assert!(remaining.as_secs() >= 1799);
        assert!(remaining.as_secs() <= 1801);
    }

    #[test]
    fn test_jwt_token_decode() {
        let token = JwtToken {
            header: "abc".to_string(),
            payload: "def".to_string(),
            signature: "ghi".to_string(),
        };
        let encoded = token.encode();
        let decoded = JwtToken::decode(&encoded).unwrap();
        assert_eq!(decoded.header, "abc");
        assert_eq!(decoded.payload, "def");
        assert_eq!(decoded.signature, "ghi");
    }

    #[test]
    fn test_jwt_token_decode_invalid() {
        let result = JwtToken::decode("only.two");
        assert!(result.is_err());
    }

    #[test]
    fn test_token_config_new() {
        let config = TokenConfig::new("my-secret", Duration::from_secs(1800));
        assert_eq!(config.secret, b"my-secret");
        assert_eq!(config.ttl, Duration::from_secs(1800));
        assert_eq!(config.issuer, "claude-code-bridge");
        assert_eq!(config.audience, "claude-code-daemon");
    }

    #[test]
    fn test_base64_encode_decode_roundtrip() {
        let data = b"Hello, world!";
        let encoded = base64_encode(data);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_base64_decode_invalid() {
        let result = base64_decode("!!!invalid!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_hmac_sha256_deterministic() {
        let key = b"test-key";
        let data = b"test-data";
        let sig1 = hmac_sha256(data, key);
        let sig2 = hmac_sha256(data, key);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_hmac_sha256_different_key() {
        let data = b"test-data";
        let sig1 = hmac_sha256(data, b"key1");
        let sig2 = hmac_sha256(data, b"key2");
        assert_ne!(sig1, sig2);
    }
}
