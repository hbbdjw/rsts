use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

pub const JWT_SECRET: &[u8] = b"rsts-secret-key-change-me";
pub const ACCESS_TOKEN_EXPIRE_MINUTES: i64 = 15;
pub const REFRESH_TOKEN_EXPIRE_DAYS: i64 = 7;

// Response Codes
pub const CODE_SUCCESS: &str = "0000";
pub const CODE_INVALID_CREDENTIALS: &str = "1001"; // Custom
pub const CODE_USER_NOT_FOUND: &str = "1002"; // Custom
pub const CODE_TOKEN_EXPIRED: &str = "9999"; // Matches VITE_SERVICE_EXPIRED_TOKEN_CODES
pub const CODE_TOKEN_INVALID: &str = "8888"; // Matches VITE_SERVICE_LOGOUT_CODES
pub const CODE_REFRESH_TOKEN_INVALID: &str = "8889"; // Matches VITE_SERVICE_LOGOUT_CODES

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub token_type: String,
    pub exp: usize,
}

pub fn create_token(
    user_id: &str,
    username: &str,
    token_type: &str,
    expire_duration: Duration,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let exp = (now + expire_duration).timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        token_type: token_type.to_string(),
        exp,
    };

    let header = Header::new(Algorithm::HS256);
    encode(&header, &claims, &EncodingKey::from_secret(JWT_SECRET))
}

pub fn verify_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.leeway = 0; // Strict expiration check

    let token_data = decode::<Claims>(token, &DecodingKey::from_secret(JWT_SECRET), &validation)?;
    Ok(token_data.claims)
}
