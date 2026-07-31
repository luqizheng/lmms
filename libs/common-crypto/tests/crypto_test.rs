use digua_common_crypto::aead::{gen_key, Aead};
use digua_common_crypto::password::{hash, verify};

#[test]
fn password_roundtrip() {
    let h = hash("secret").unwrap();
    assert!(verify("secret", &h).unwrap());
    assert!(!verify("wrong", &h).unwrap());
}

#[test]
fn password_hash_is_consistent() {
    let h1 = hash("same-password").unwrap();
    let h2 = hash("same-password").unwrap();
    assert!(h1 != h2);
    assert!(verify("same-password", &h1).unwrap());
    assert!(verify("same-password", &h2).unwrap());
}

#[test]
fn password_accepts_empty_string() {
    let h = hash("").unwrap();
    assert!(verify("", &h).unwrap());
}

#[test]
fn aead_seal_open_roundtrip() {
    let key = gen_key();
    let aead = Aead::new(&key);
    let ct = aead.seal(b"hello", b"aad").unwrap();
    let pt = aead.open(&ct, b"aad").unwrap();
    assert_eq!(pt, b"hello");
}

#[test]
fn aead_different_keys_fail() {
    let key1 = gen_key();
    let key2 = gen_key();
    let aead1 = Aead::new(&key1);
    let aead2 = Aead::new(&key2);
    let ct = aead1.seal(b"data", b"aad").unwrap();
    assert!(aead2.open(&ct, b"aad").is_err());
}

#[test]
fn aead_wrong_aad_fails() {
    let key = gen_key();
    let aead = Aead::new(&key);
    let ct = aead.seal(b"data", b"correct-aad").unwrap();
    assert!(aead.open(&ct, b"wrong-aad").is_err());
}

#[test]
fn aead_too_short_blob_fails() {
    let key = gen_key();
    let aead = Aead::new(&key);
    assert!(aead.open(b"short", b"aad").is_err());
}

#[test]
fn aead_empty_plaintext() {
    let key = gen_key();
    let aead = Aead::new(&key);
    let ct = aead.seal(b"", b"aad").unwrap();
    let pt = aead.open(&ct, b"aad").unwrap();
    assert_eq!(pt, b"");
}
