use serde::{de::DeserializeOwned, Serialize};

pub trait Event: Serialize + DeserializeOwned + Send + Sync + 'static {
    fn event_type() -> &'static str;
    fn schema_version() -> u32 {
        1
    }
}
