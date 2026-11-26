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
    // 契约（与 is_strong_password 实现、accepts_3_of_4 用例一致）：
    // len(bytes) >= 8 且字符类别（小写/大写/数字/符号）>= 3。
    assert!(!is_strong_password("aaaaaaaa"));
    assert!(!is_strong_password("Aa1!aa"));
    assert!(!is_strong_password("Aa1aaaa"));
    // 8 字节但只有 2 类（小写+数字 / 大写+数字）→ 弱，拒绝。
    assert!(!is_strong_password("aaaa1234"));
    assert!(!is_strong_password("AAAA1111"));
    assert!(!is_strong_password(""));
    assert!(!is_strong_password("a"));
    assert!(!is_strong_password("Aa1!"));
}

#[test]
fn strong_password_accepts_3_of_4_classes() {
    // 3 类 + 8 字节 → 强（原样例 "Abcdef1"/"Abcdef!" 只有 7 字节，低于
    // len>=8 契约，属测试笔误；该文件此前从未被任何 CI target 执行）。
    assert!(is_strong_password("Abcdef12"));
    assert!(is_strong_password("Abcdef1!"));
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
