use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rand::rngs::OsRng;

/// Dev 用的明文 client_secret；启动时如果现存 hash 验证不通过就用它重算并替换。
/// 生产环境应通过环境变量注入真 hash，本函数不参与。
pub const DEFAULT_DEV_CLIENT_SECRET: &str = "dev-gateway-secret";

pub fn hash_client_secret(plain: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .expect("hash client secret")
        .to_string()
}

/// 验证现存 hash 是否与默认明文一致；不一致则重算并原地替换。
pub fn maybe_upgrade_client_secret(existing_hash: &mut String, default_plain: &str) {
    let verified = PasswordHash::new(existing_hash)
        .ok()
        .and_then(|ph| {
            Argon2::default()
                .verify_password(default_plain.as_bytes(), &ph)
                .ok()
        })
        .is_some();
    if !verified {
        *existing_hash = hash_client_secret(default_plain);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify_dev_secret() {
        let h = hash_client_secret(DEFAULT_DEV_CLIENT_SECRET);
        assert!(PasswordHash::new(&h).is_ok());
    }

    #[test]
    fn upgrade_replaces_placeholder_hash() {
        let mut h = "$argon2id$v=19$m=65536,t=3,p=4$placeholder$invalidsaltvalue".into();
        maybe_upgrade_client_secret(&mut h, DEFAULT_DEV_CLIENT_SECRET);
        let ph = PasswordHash::new(&h).unwrap();
        assert!(Argon2::default()
            .verify_password(DEFAULT_DEV_CLIENT_SECRET.as_bytes(), &ph)
            .is_ok());
    }
}
