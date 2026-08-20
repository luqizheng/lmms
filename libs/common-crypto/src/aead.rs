pub use aes_gcm::Aes256Gcm;
use aes_gcm::{
    aead::{Aead as _, AeadCore, KeyInit, OsRng},
    Key, Nonce,
};
use digua_common_error::{AppError, AppResult};

pub fn gen_key() -> [u8; 32] {
    use rand::RngCore;
    let mut k = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut k);
    k
}

pub struct Aead {
    cipher: Aes256Gcm,
}

impl Aead {
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key)),
        }
    }
    pub fn seal(&self, plaintext: &[u8], aad: &[u8]) -> AppResult<Vec<u8>> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ct = self
            .cipher
            .encrypt(
                &nonce,
                aes_gcm::aead::Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|e| AppError::internal("AEAD_SEAL", e.to_string()))?;
        let mut out = nonce.to_vec();
        out.extend_from_slice(&ct);
        Ok(out)
    }
    pub fn open(&self, blob: &[u8], aad: &[u8]) -> AppResult<Vec<u8>> {
        if blob.len() < 12 {
            return Err(AppError::bad_request("AEAD_LEN", "blob too short"));
        }
        let (n, c) = blob.split_at(12);
        let nonce = Nonce::from_slice(n);
        self.cipher
            .decrypt(nonce, aes_gcm::aead::Payload { msg: c, aad })
            .map_err(|e| AppError::internal("AEAD_OPEN", e.to_string()))
    }
}
