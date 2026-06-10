use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;

pub fn hash(p: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let phc = Argon2::default()
        .hash_password(p.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("argon2 hash: {e}"))?;
    Ok(phc.to_string())
}

pub fn verify(p: &str, h: &str) -> anyhow::Result<bool> {
    let parsed = PasswordHash::new(h).map_err(|e| anyhow::anyhow!("hash parse: {e}"))?;
    Ok(Argon2::default()
        .verify_password(p.as_bytes(), &parsed)
        .is_ok())
}
