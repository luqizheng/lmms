use common_crypto::password::{hash, verify};
use common_crypto::aead::{Aead, gen_key};

#[test]
fn password_roundtrip() {
    let h = hash("secret").unwrap();
    assert!(verify("secret", &h).unwrap());
    assert!(!verify("wrong", &h).unwrap());
}

#[test]
fn aead_seal_open_roundtrip() {
    let key = gen_key();
    let aead = Aead::new(&key);
    let ct = aead.seal(b"hello", b"aad").unwrap();
    let pt = aead.open(&ct, b"aad").unwrap();
    assert_eq!(pt, b"hello");
}