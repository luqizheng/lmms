use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct RequestId(String);

impl RequestId {
    pub fn from_header(h: Option<&str>) -> Self {
        match h {
            Some(v) if !v.is_empty() => Self(v.to_string()),
            _ => Self(Uuid::new_v4().to_string()),
        }
    }
    pub fn as_str(&self) -> &str { &self.0 }
}