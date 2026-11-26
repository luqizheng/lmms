use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct RequestId(String);

impl RequestId {
    pub fn from_header(h: Option<&str>) -> Self {
        match h {
            // 仅接受非空、非纯空白的值：全空白 header（如 "   "）与缺失等价，
            // 生成新的 uuid（log_test.rs 的契约；透传空白会让响应头写入失败，
            // 因为 HTTP header value 不允许裸空白）。
            Some(v) if !v.trim().is_empty() => Self(v.to_string()),
            _ => Self(Uuid::new_v4().to_string()),
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
