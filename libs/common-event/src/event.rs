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
///
/// CONSUME-ONLY: do not `publish(Envelope<serde_json::Value>)` — the publisher
/// uses `P::event_type()` as the routing key, so a `Value` payload would route
/// to the literal `"*"` key. Publish typed events (the generated structs).
impl Event for serde_json::Value {
    fn event_type() -> &'static str {
        "*"
    }
}
