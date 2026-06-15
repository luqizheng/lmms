use common_error::{AppError, AppResult};
use rsa::pkcs1v15::{Signature, SigningKey, VerifyingKey};
use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::sha2::Sha256;
use rsa::signature::{RandomizedSigner, SignatureEncoding, Verifier};
use rsa::traits::PublicKeyParts as _;
use rsa::{Oaep, Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey as InnerRsaPublicKey};

#[derive(Debug, Clone, Copy)]
pub enum RsaScheme {
    OaepSha256,
    Pkcs1Sha256,
}

pub struct RsaKeyPair {
    inner: RsaPrivateKey,
    public_key: InnerRsaPublicKey,
}

impl RsaKeyPair {
    pub fn generate(bits: usize) -> AppResult<Self> {
        let mut rng = rand::rngs::OsRng;
        let inner = RsaPrivateKey::new(&mut rng, bits)
            .map_err(|e| AppError::internal("RSA_GENERATE", e.to_string()))?;
        let public_key = InnerRsaPublicKey::from(&inner);

        Ok(Self { inner, public_key })
    }

    pub fn from_pkcs8_pem(pem: &str) -> AppResult<Self> {
        let inner = RsaPrivateKey::from_pkcs8_pem(pem)
            .map_err(|e| AppError::internal("RSA_PARSE_KEY", e.to_string()))?;
        let public_key = InnerRsaPublicKey::from(&inner);

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
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| AppError::internal("RSA_PUBLIC_PEM", e.to_string()))
    }

    pub fn modulus_size(&self) -> usize {
        self.public_key.size()
    }

    pub fn encrypt(&self, plaintext: &[u8], scheme: RsaScheme) -> AppResult<Vec<u8>> {
        let mut rng = rand::rngs::OsRng;
        match scheme {
            RsaScheme::OaepSha256 => self
                .public_key
                .encrypt(&mut rng, Oaep::new::<Sha256>(), plaintext)
                .map_err(|e| AppError::internal("RSA_ENCRYPT", e.to_string())),
            RsaScheme::Pkcs1Sha256 => self
                .public_key
                .encrypt(&mut rng, Pkcs1v15Encrypt, plaintext)
                .map_err(|e| AppError::internal("RSA_ENCRYPT", e.to_string())),
        }
    }

    pub fn decrypt(&self, ciphertext: &[u8], scheme: RsaScheme) -> AppResult<Vec<u8>> {
        match scheme {
            RsaScheme::OaepSha256 => self
                .inner
                .decrypt(Oaep::new::<Sha256>(), ciphertext)
                .map_err(|e| AppError::internal("RSA_DECRYPT", e.to_string())),
            RsaScheme::Pkcs1Sha256 => self
                .inner
                .decrypt(Pkcs1v15Encrypt, ciphertext)
                .map_err(|e| AppError::internal("RSA_DECRYPT", e.to_string())),
        }
    }

    pub fn sign(&self, message: &[u8], _scheme: RsaScheme) -> AppResult<Vec<u8>> {
        let signing_key = SigningKey::<Sha256>::new(self.inner.clone());
        let mut rng = rand::rngs::OsRng;
        let signature = signing_key.sign_with_rng(&mut rng, message);
        Ok(signature.to_bytes().to_vec())
    }
}

pub struct RsaPublicKeyWrapper {
    inner: InnerRsaPublicKey,
}

impl RsaPublicKeyWrapper {
    pub fn from_pem(pem: &str) -> AppResult<Self> {
        let inner = InnerRsaPublicKey::from_public_key_pem(pem)
            .map_err(|e| AppError::internal("RSA_PARSE_PUB", e.to_string()))?;

        Ok(Self { inner })
    }

    pub fn encrypt(&self, plaintext: &[u8], scheme: RsaScheme) -> AppResult<Vec<u8>> {
        let mut rng = rand::rngs::OsRng;
        match scheme {
            RsaScheme::OaepSha256 => self
                .inner
                .encrypt(&mut rng, Oaep::new::<Sha256>(), plaintext)
                .map_err(|e| AppError::internal("RSA_ENCRYPT", e.to_string())),
            RsaScheme::Pkcs1Sha256 => self
                .inner
                .encrypt(&mut rng, Pkcs1v15Encrypt, plaintext)
                .map_err(|e| AppError::internal("RSA_ENCRYPT", e.to_string())),
        }
    }

    pub fn verify(&self, message: &[u8], signature: &[u8], _scheme: RsaScheme) -> AppResult<bool> {
        let verifying_key = VerifyingKey::<Sha256>::new(self.inner.clone());
        let sig = Signature::try_from(signature)
            .map_err(|e| AppError::internal("RSA_SIG_PARSE", e.to_string()))?;
        Ok(verifying_key.verify(message, &sig).is_ok())
    }
}

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
