use common::password::{hash, verify};

#[test]
fn hash_and_verify_roundtrip() {
    let h = hash("hunter2").unwrap();
    assert!(verify("hunter2", &h).unwrap());
    assert!(!verify("wrong", &h).unwrap());
}

#[test]
fn different_inputs_yield_different_hashes() {
    let h1 = hash("a").unwrap();
    let h2 = hash("a").unwrap();
    assert_ne!(h1, h2);
}
