// Auto-generated from contracts/events.yaml
// DO NOT EDIT MANUALLY

use serde::{Deserialize, Serialize};

use crate::envelope::Envelope;
use crate::event::Event;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IamUserCreated {
    pub user_id: String,
    pub username: String,
    pub email: Option<String>,
    pub org_unit_id: Option<String>,
}

impl Event for IamUserCreated {
    fn event_type() -> &'static str { "iam.user.created" }
    fn schema_version() -> u32 { 1 }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IamUserDeleted {
    pub user_id: String,
    pub deleted_by: String,
}

impl Event for IamUserDeleted {
    fn event_type() -> &'static str { "iam.user.deleted" }
    fn schema_version() -> u32 { 1 }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IamLoginSucceeded {
    pub user_id: String,
    pub ip: String,
    pub user_agent: Option<String>,
    pub method: String,
}

impl Event for IamLoginSucceeded {
    fn event_type() -> &'static str { "iam.login.succeeded" }
    fn schema_version() -> u32 { 1 }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IamLoginFailed {
    pub username: String,
    pub user_id: Option<String>,
    pub ip: String,
    pub user_agent: Option<String>,
    pub reason: String,
}

impl Event for IamLoginFailed {
    fn event_type() -> &'static str { "iam.login.failed" }
    fn schema_version() -> u32 { 1 }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", content = "payload", rename_all = "snake_case")]
pub enum IamEventEnum {
    Unknown {
        event_type: String,
        envelope: serde_json::Value,
    },
    IamUserCreated(Envelope<IamUserCreated>),
    IamUserDeleted(Envelope<IamUserDeleted>),
    IamLoginSucceeded(Envelope<IamLoginSucceeded>),
    IamLoginFailed(Envelope<IamLoginFailed>),
}
