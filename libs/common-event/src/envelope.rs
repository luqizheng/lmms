use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::event::Event;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(serialize = "P: Serialize", deserialize = "P: Event"))]
pub struct Envelope<P: Event> {
    pub event_id: Uuid,
    pub event_type: String,
    pub schema_version: u32,
    pub occurred_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tenant_id: Option<String>,
    pub producer: String,
    pub payload: P,
}

impl<P: Event> Envelope<P> {
    pub fn new(producer: impl Into<String>, payload: P) -> Self {
        Self {
            event_id: Uuid::now_v7(),
            event_type: P::event_type().to_string(),
            schema_version: P::schema_version(),
            occurred_at: Utc::now(),
            trace_id: None,
            tenant_id: None,
            producer: producer.into(),
            payload,
        }
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }
}
