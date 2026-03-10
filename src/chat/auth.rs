use anyhow::{bail, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// JWT claims payload.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub user_id: i64,
    pub username: String,
    pub is_bot: bool,
    pub exp: u64,
}

/// Hash a password with argon2.
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("password hashing failed: {e}"))?;
    Ok(hash.to_string())
}

/// Verify a password against a hash.
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let parsed =
        PasswordHash::new(hash).map_err(|e| anyhow::anyhow!("invalid password hash: {e}"))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Create a JWT token for a user.
pub fn create_token(secret: &str, user_id: i64, username: &str, is_bot: bool) -> Result<String> {
    if secret.is_empty() {
        bail!("JWT secret cannot be empty");
    }

    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 86400; // 24 hours

    let claims = Claims {
        user_id,
        username: username.to_string(),
        is_bot,
        exp,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    Ok(token)
}

/// Validate a JWT token and return claims.
pub fn validate_token(secret: &str, token: &str) -> Result<Claims> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;

    Ok(token_data.claims)
}

/// Axum middleware that validates JWT from Authorization header.
/// Injects Claims into request extensions on success.
pub async fn jwt_middleware(
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::{http::StatusCode, response::IntoResponse, Json};
    use serde_json::json;

    let secret = match std::env::var("TEIDE_CHAT_SECRET") {
        Ok(s) if !s.is_empty() => s,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"ok": false, "error": "server_misconfigured"})),
            )
                .into_response();
        }
    };

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"ok": false, "error": "not_authed"})),
            )
                .into_response();
        }
    };

    match validate_token(&secret, token) {
        Ok(claims) => {
            request.extensions_mut().insert(claims);
            next.run(request).await
        }
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": "invalid_auth"})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hash_and_verify() {
        let hash = hash_password("mysecret").unwrap();
        assert!(verify_password("mysecret", &hash).unwrap());
        assert!(!verify_password("wrongpass", &hash).unwrap());
    }

    #[test]
    fn test_jwt_roundtrip() {
        let secret = "test-secret-key";
        let token = create_token(secret, 42, "alice", false).unwrap();
        let claims = validate_token(secret, &token).unwrap();
        assert_eq!(claims.user_id, 42);
        assert_eq!(claims.username, "alice");
        assert!(!claims.is_bot);
    }

    #[test]
    fn test_jwt_invalid_secret() {
        let token = create_token("secret1", 1, "bob", false).unwrap();
        assert!(validate_token("secret2", &token).is_err());
    }

    #[test]
    fn test_jwt_empty_secret_rejected() {
        assert!(create_token("", 1, "bob", false).is_err());
    }
}
