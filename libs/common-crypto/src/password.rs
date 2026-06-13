use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use common_error::{AppError, AppResult};

pub fn hash(plain: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let h = Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| AppError::internal("PWD_HASH", e.to_string()))?;
    Ok(h.to_string())
}

pub fn verify(plain: &str, hash_str: &str) -> AppResult<bool> {
    let parsed =
        PasswordHash::new(hash_str).map_err(|e| AppError::internal("PWD_PARSE", e.to_string()))?;
    Ok(Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok())
}
