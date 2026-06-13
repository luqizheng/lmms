use common_validator::{is_cn_mobile, is_strong_password};

#[test]
fn cn_mobile_accepts_valid() {
    assert!(is_cn_mobile("13912345678"));
    assert!(!is_cn_mobile("12012345678"));
    assert!(!is_cn_mobile("1391234567"));
}

#[test]
fn strong_password_requires_classes_and_length() {
    assert!(is_strong_password("Aa1!aaaa"));
    assert!(!is_strong_password("aaaaaaaa"));
    assert!(!is_strong_password("Aa1!aa"));
}