pub fn is_cn_mobile(s: &str) -> bool {
    if s.len() != 11 { return false; }
    if !s.chars().all(|c| c.is_ascii_digit()) { return false; }
    let b = s.as_bytes();
    b[0] == b'1' && matches!(b[1], b'3'..=b'9')
}

pub fn is_strong_password(s: &str) -> bool {
    if s.len() < 8 { return false; }
    let has_lower = s.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = s.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = s.chars().any(|c| c.is_ascii_digit());
    let has_sym   = s.chars().any(|c| !c.is_ascii_alphanumeric());
    [has_lower, has_upper, has_digit, has_sym].iter().filter(|x| **x).count() >= 3
}