use serde::{de::DeserializeOwned, Serialize};

pub trait Event: Serialize + DeserializeOwned + Send + Sync + 'static {
    fn event_type() -> &'static str;
    fn schema_version() -> u32 {
        1
    }
}

/// Generic payload for consumers that handle heterogeneous events and only
/// need the raw JSON payload plus the envelope metadata (e.g. the audit
/// service, which stores every iam.* event without caring about its concrete
/// type). The `event_type()` sentinel is never used for deserialization —
/// serde reads the real `event_type` from the envelope's own field.
impl Event for serde_json::Value {
    fn event_type() -> &'static str {
        "*"
    }
}
