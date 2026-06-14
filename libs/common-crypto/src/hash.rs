use sha2::{Sha256 as RustSha256, Digest};

pub trait Hash {
    fn hash(data: &[u8]) -> Vec<u8>;
    fn hash_hex(data: &[u8]) -> String;
}

pub struct Sha256;

impl Hash for Sha256 {
    fn hash(data: &[u8]) -> Vec<u8> {
        let mut hasher = RustSha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }

    fn hash_hex(data: &[u8]) -> String {
        hex::encode(Self::hash(data))
    }
}

pub struct Blake3;

impl Hash for Blake3 {
    fn hash(data: &[u8]) -> Vec<u8> {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }

    fn hash_hex(data: &[u8]) -> String {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(data);
        hasher.finalize().to_hex().to_string()
    }
}

pub type Hash = dyn Hash;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256() {
        let data = b"Hello, SHA256!";
        let hash = Sha256::hash(data);
        assert_eq!(hash.len(), 32);
        assert_eq!(Sha256::hash_hex(data).len(), 64);
    }

    #[test]
    fn test_blake3() {
        let data = b"Hello, BLAKE3!";
        let hash = Blake3::hash(data);
        assert_eq!(hash.len(), 32);
        assert_eq!(Blake3::hash_hex(data).len(), 64);
    }

    #[test]
    fn test_sha256_consistency() {
        let data = b"test";
        let hash1 = Sha256::hash(data);
        let hash2 = Sha256::hash(data);
        assert_eq!(hash1, hash2);
    }
}
