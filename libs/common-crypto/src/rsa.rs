use common_error::{AppError, AppResult};
use rsa::{
    pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey, EncodeRsaPrivateKey, EncodeRsaPublicKey},
    pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding},
    sha2::Sha256,
    SignatureEncoding,
};
use rsa::{PaddingScheme, PublicKeyParts, RsaPrivateKey, RsaPublicKey};

#[derive(Debug, Clone, Copy)]
pub enum RsaScheme {
    OaepSha256,
    Pkcs1Sha256,
}

pub struct RsaKeyPair {
    inner: RsaPrivateKey,
    public_key: RsaPublicKey,
}

impl RsaKeyPair {
    pub fn generate(bits: usize) -> AppResult<Self> {
        let mut rng = rand::rngs::OsRng;
        let inner = RsaPrivateKey::new(&mut rng, bits)
            .map_err(|e| AppError::internal("RSA_GENERATE", e.to_string()))?;
        let public_key = RsaPublicKey::from(&inner);

        Ok(Self { inner, public_key })
    }

    pub fn from_pkcs8_pem(pem: &str) -> AppResult<Self> {
        let inner = RsaPrivateKey::from_pkcs8_pem(pem)
            .map_err(|e| AppError::internal("RSA_PARSE_KEY", e.to_string()))?;
        let public_key = RsaPublicKey::from(&inner);

        Ok(Self { inner, public_key })
    }

    pub fn private_key_pem(&self) -> AppResult<String> {
        self.inner
            .to_pkcs8_pem(LineEnding::LF)
            .map(|p| p.to_string())
            .map_err(|e| AppError::internal("RSA_PRIVATE_PEM", e.to_string()))
    }

    pub fn public_key_pem(&self) -> AppResult<String> {
        self.public_key
            .to_pkcs8_pem(LineEnding::LF)
            .map(|p| p.to_string())
            .map_err(|e| AppError::internal("RSA_PUBLIC_PEM", e.to_string()))
    }

    pub fn encrypt(&self, plaintext: &[u8], scheme: RsaScheme) -> AppResult<Vec<u8>> {
        let padding = match scheme {
            RsaScheme::OaepSha256 => PaddingScheme::new_oaep::<Sha256>(),
            RsaScheme::Pkcs1Sha256 => PaddingScheme::new_pkcs1v15_encrypt(),
        };

        self.public_key
            .encrypt(rand::rngs::OsRng, padding, plaintext)
            .map_err(|e| AppError::internal("RSA_ENCRYPT", e.to_string()))
    }

    pub fn decrypt(&self, ciphertext: &[u8], scheme: RsaScheme) -> AppResult<Vec<u8>> {
        let padding = match scheme {
            RsaScheme::OaepSha256 => PaddingScheme::new_oaep::<Sha256>(),
            RsaScheme::Pkcs1Sha256 => PaddingScheme::new_pkcs1v15_encrypt(),
        };

        self.inner
            .decrypt(padding, ciphertext)
            .map_err(|e| AppError::internal("RSA_DECRYPT", e.to_string()))
    }

    pub fn sign(&self, message: &[u8], scheme: RsaScheme) -> AppResult<Vec<u8>> {
        let padding = match scheme {
            RsaScheme::OaepSha256 => PaddingScheme::new_pkcs1v15_sign(rsa::sha2::Sha256::new()),
            RsaScheme::Pkcs1Sha256 => PaddingScheme::new_pkcs1v15_sign(rsa::sha2::Sha256::new()),
        };

        let signature = self
            .inner
            .sign(padding, message)
            .map_err(|e| AppError::internal("RSA_SIGN", e.to_string()))?;

        Ok(signature.to_vec())
    }
}

pub struct RsaPublicKeyWrapper {
    inner: RsaPublicKey,
}

impl RsaPublicKeyWrapper {
    pub fn from_pem(pem: &str) -> AppResult<Self> {
        let inner = RsaPublicKey::from_public_key_pem(pem)
            .map_err(|e| AppError::internal("RSA_PARSE_PUB", e.to_string()))?;

        Ok(Self { inner })
    }

    pub fn encrypt(&self, plaintext: &[u8], scheme: RsaScheme) -> AppResult<Vec<u8>> {
        let padding = match scheme {
            RsaScheme::OaepSha256 => PaddingScheme::new_oaep::<Sha256>(),
            RsaScheme::Pkcs1Sha256 => PaddingScheme::new_pkcs1v15_encrypt(),
        };

        self.inner
            .encrypt(rand::rngs::OsRng, padding, plaintext)
            .map_err(|e| AppError::internal("RSA_ENCRYPT", e.to_string()))
    }

    pub fn verify(&self, message: &[u8], signature: &[u8], scheme: RsaScheme) -> AppResult<bool> {
        let padding = match scheme {
            RsaScheme::OaepSha256 => PaddingScheme::new_pkcs1v15_sign(rsa::sha2::Sha256::new()),
            RsaScheme::Pkcs1Sha256 => PaddingScheme::new_pkcs1v15_sign(rsa::sha2::Sha256::new()),
        };

        let sig = rsa::Signature::try_from(signature)
            .map_err(|e| AppError::internal("RSA_SIG_PARSE", e.to_string()))?;

        Ok(self.inner.verify(padding, message, &sig).is_ok())
    }
}

pub type RsaKeyPair = RsaKeyPair;
pub type RsaPublicKey = RsaPublicKeyWrapper;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsa_generate() {
        let keypair = RsaKeyPair::generate(2048).unwrap();
        assert!(!keypair.private_key_pem().unwrap().is_empty());
        assert!(!keypair.public_key_pem().unwrap().is_empty());
    }

    #[test]
    fn test_rsa_encrypt_decrypt() {
        let keypair = RsaKeyPair::generate(2048).unwrap();
        let plaintext = b"Hello, RSA!";
        
        let ciphertext = keypair.encrypt(plaintext, RsaScheme::OaepSha256).unwrap();
        let decrypted = keypair.decrypt(&ciphertext, RsaScheme::OaepSha256).unwrap();
        
        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_rsa_sign_verify() {
        let keypair = RsaKeyPair::generate(2048).unwrap();
        let message = b"Hello, Sign!";
        
        let signature = keypair.sign(message, RsaScheme::Pkcs1Sha256).unwrap();
        
        let public_key = RsaPublicKey::from_pem(&keypair.public_key_pem().unwrap()).unwrap();
        let valid = public_key.verify(message, &signature, RsaScheme::Pkcs1Sha256).unwrap();
        
        assert!(valid);
    }
}
