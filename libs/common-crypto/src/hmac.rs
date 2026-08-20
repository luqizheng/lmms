use hmac::{Hmac as HmacCore, Mac};
use sha2::{Sha256, Sha512};

pub trait Hmac: Clone {
    fn mac(&self, key: &[u8], message: &[u8]) -> Vec<u8>;
    fn verify(&self, key: &[u8], message: &[u8], expected: &[u8]) -> bool;
}

#[derive(Clone)]
pub struct HmacSha256;

impl Hmac for HmacSha256 {
    fn mac(&self, key: &[u8], message: &[u8]) -> Vec<u8> {
        let mut mac =
            <HmacCore<Sha256> as Mac>::new_from_slice(key).expect("HMAC-SHA256 key length");
        mac.update(message);
        mac.finalize().into_bytes().to_vec()
    }

    fn verify(&self, key: &[u8], message: &[u8], expected: &[u8]) -> bool {
        constant_time_eq(&self.mac(key, message), expected)
    }
}

#[derive(Clone)]
pub struct HmacSha512;

impl Hmac for HmacSha512 {
    fn mac(&self, key: &[u8], message: &[u8]) -> Vec<u8> {
        let mut mac =
            <HmacCore<Sha512> as Mac>::new_from_slice(key).expect("HMAC-SHA512 key length");
        mac.update(message);
        mac.finalize().into_bytes().to_vec()
    }

    fn verify(&self, key: &[u8], message: &[u8], expected: &[u8]) -> bool {
        constant_time_eq(&self.mac(key, message), expected)
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_sha256() {
        let hmac = HmacSha256;
        let message = b"Hello, HMAC!";

        let digest = hmac.mac(b"secret", message);
        assert_eq!(digest.len(), 32);

        assert!(hmac.verify(b"secret", message, &digest));
        assert!(!hmac.verify(b"wrong", message, &digest));
    }

    #[test]
    fn test_hmac_sha512() {
        let hmac = HmacSha512;
        let message = b"Hello, HMAC!";

        let digest = hmac.mac(b"secret", message);
        assert_eq!(digest.len(), 64);

        assert!(hmac.verify(b"secret", message, &digest));
    }
}
