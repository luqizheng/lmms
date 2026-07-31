use digua_common_validator::{is_cn_mobile, is_strong_password};

#[test]
fn cn_mobile_accepts_valid() {
    assert!(is_cn_mobile("13912345678"));
    assert!(is_cn_mobile("15012345678"));
    assert!(is_cn_mobile("18912345678"));
}

#[test]
fn cn_mobile_rejects_invalid() {
    assert!(!is_cn_mobile("12012345678"));
    assert!(!is_cn_mobile("1391234567"));
    assert!(!is_cn_mobile("139123456789"));
    assert!(!is_cn_mobile("abc12345678"));
    assert!(!is_cn_mobile(""));
}

#[test]
fn strong_password_requires_classes_and_length() {
    assert!(is_strong_password("Aa1!aaaa"));
    assert!(is_strong_password("Password1!"));
    assert!(is_strong_password("1234Abc!"));
}

#[test]
fn strong_password_rejects_weak() {
    assert!(!is_strong_password("aaaaaaaa"));
    assert!(!is_strong_password("Aa1!aa"));
    assert!(!is_strong_password("Aa1aaaa"));
    assert!(!is_strong_password("aa1!aaaa"));
    assert!(!is_strong_password("AA1!AAAA"));
    assert!(!is_strong_password(""));
    assert!(!is_strong_password("a"));
    assert!(!is_strong_password("Aa1!"));
}

#[test]
fn strong_password_accepts_3_of_4_classes() {
    assert!(is_strong_password("Abcdef1"));
    assert!(is_strong_password("Abcdef!"));
    assert!(is_strong_password("abcd123!"));
}

#[test]
fn cn_mobile_handles_non_ascii() {
    assert!(!is_cn_mobile("１３９１２３４５６７８"));
    assert!(!is_cn_mobile("１２３"));
}

#[test]
fn strong_password_accepts_various_symbols() {
    assert!(is_strong_password("Aa1@aaaa"));
    assert!(is_strong_password("Aa1#aaaa"));
    assert!(is_strong_password("Aa1$aaaa"));
    assert!(is_strong_password("Aa1%aaaa"));
    assert!(is_strong_password("Aa1^aaaa"));
    assert!(is_strong_password("Aa1&aaaa"));
    assert!(is_strong_password("Aa1*aaaa"));
}
