use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, SaltString},
    Argon2, Algorithm, Version,
};
use common_error::{AppError, AppResult};

pub use argon2::password_hash::PasswordHasher;

pub fn hash(plain: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let params = argon2::Params::default();
    let argon2 = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        params,
    );
    let h = argon2
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| AppError::internal("PWD_HASH", e.to_string()))?;
    Ok(h.to_string())
}

pub fn verify(plain: &str, hash_str: &str) -> AppResult<bool> {
    let parsed =
        PasswordHash::new(hash_str).map_err(|e| AppError::internal("PWD_PARSE", e.to_string()))?;
    Ok(parsed.verify_password(&[&Argon2::default(), &Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2::Params::default())], plain.as_bytes()).is_ok())
}