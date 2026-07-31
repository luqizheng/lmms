use digua_common_error::AppResult;
use async_trait::async_trait;

#[async_trait]
pub trait KmsClient: Send + Sync {
    async fn encrypt(&self, key_id: &str, plaintext: &[u8]) -> AppResult<Vec<u8>>;
    async fn decrypt(&self, key_id: &str, ciphertext: &[u8]) -> AppResult<Vec<u8>>;
    async fn generate_data_key(&self, key_id: &str) -> AppResult<DataKey>;
}

#[derive(Debug, Clone)]
pub struct DataKey {
    pub plaintext: Vec<u8>,
    pub encrypted: Vec<u8>,
}

pub struct LocalKms {
    master_key: Vec<u8>,
}

impl LocalKms {
    pub fn new(master_key: Vec<u8>) -> Self {
        Self { master_key }
    }

    fn derive_key(&self, info: &[u8]) -> Vec<u8> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(&self.master_key);
        hasher.update(info);
        hasher.finalize().to_vec()
    }
}

#[async_trait]
impl KmsClient for LocalKms {
    async fn encrypt(&self, key_id: &str, plaintext: &[u8]) -> AppResult<Vec<u8>> {
        let derived_key = self.derive_key(key_id.as_bytes());

        let mut key = [0u8; 32];
        let n = derived_key.len().min(32);
        key[..n].copy_from_slice(&derived_key[..n]);
        let aead = crate::aead::Aead::new(&key);
        aead.seal(plaintext, key_id.as_bytes())
    }

    async fn decrypt(&self, key_id: &str, ciphertext: &[u8]) -> AppResult<Vec<u8>> {
        let derived_key = self.derive_key(key_id.as_bytes());

        let mut key = [0u8; 32];
        let n = derived_key.len().min(32);
        key[..n].copy_from_slice(&derived_key[..n]);
        let aead = crate::aead::Aead::new(&key);
        aead.open(ciphertext, key_id.as_bytes())
    }

    async fn generate_data_key(&self, key_id: &str) -> AppResult<DataKey> {
        use rand::RngCore;
        let mut plaintext = vec![0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut plaintext);
        
        let encrypted = self.encrypt(key_id, &plaintext).await?;
        
        Ok(DataKey { plaintext, encrypted })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_local_kms_encrypt_decrypt() {
        let kms = LocalKms::new(b"master-key-32-bytes-long-here!".to_vec());
        
        let plaintext = b"Secret message";
        let ciphertext = kms.encrypt("my-key", plaintext).await.unwrap();
        let decrypted = kms.decrypt("my-key", &ciphertext).await.unwrap();
        
        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[tokio::test]
    async fn test_generate_data_key() {
        let kms = LocalKms::new(b"master-key-32-bytes-long-here!".to_vec());
        
        let data_key = kms.generate_data_key("my-key").await.unwrap();
        assert_eq!(data_key.plaintext.len(), 32);
        assert!(!data_key.encrypted.is_empty());
    }
}
