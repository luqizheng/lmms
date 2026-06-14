use common_error::AppResult;

pub trait Hmac: Clone {
    fn mac(&self, key: &[u8], message: &[u8]) -> Vec<u8>;
    fn verify(&self, key: &[u8], message: &[u8], expected: &[u8]) -> bool;
}

#[derive(Clone)]
pub struct HmacSha256 {
    inner: hmac_sha256::HMAC,
}

impl HmacSha256 {
    pub fn new(key: &[u8]) -> Self {
        Self {
            inner: hmac_sha256::HMAC::new(key),
        }
    }
}

impl Hmac for HmacSha256 {
    fn mac(&self, key: &[u8], message: &[u8]) -> Vec<u8> {
        let mut hmac = hmac_sha256::HMAC::new(key);
        hmac.write(message);
        hmac.finalize().to_vec()
    }

    fn verify(&self, key: &[u8], message: &[u8], expected: &[u8]) -> bool {
        let computed = self.mac(key, message);
        constant_time_eq(&computed, expected)
    }
}

#[derive(Clone)]
pub struct HmacSha512 {
    inner: hmac_sha512::HMAC,
}

impl HmacSha512 {
    pub fn new(key: &[u8]) -> Self {
        Self {
            inner: hmac_sha512::HMAC::new(key),
        }
    }
}

impl Hmac for HmacSha512 {
    fn mac(&self, key: &[u8], message: &[u8]) -> Vec<u8> {
        let mut hmac = hmac_sha512::HMAC::new(key);
        hmac.write(message);
        hmac.finalize().to_vec()
    }

    fn verify(&self, key: &[u8], message: &[u8], expected: &[u8]) -> bool {
        let computed = self.mac(key, message);
        constant_time_eq(&computed, expected)
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

pub type Hmac = dyn Hmac;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_sha256() {
        let hmac = HmacSha256::new(b"secret");
        let message = b"Hello, HMAC!";
        
        let digest = hmac.mac(b"secret", message);
        assert_eq!(digest.len(), 32);
        
        assert!(hmac.verify(b"secret", message, &digest));
        assert!(!hmac.verify(b"wrong", message, &digest));
    }

    #[test]
    fn test_hmac_sha512() {
        let hmac = HmacSha512::new(b"secret");
        let message = b"Hello, HMAC!";
        
        let digest = hmac.mac(b"secret", message);
        assert_eq!(digest.len(), 64);
        
        assert!(hmac.verify(b"secret", message, &digest));
    }
}
