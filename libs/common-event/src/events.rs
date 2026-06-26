// Auto-generated from contracts/events.yaml
// DO NOT EDIT MANUALLY

use serde::{Deserialize, Serialize};

use crate::envelope::Envelope;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IamUserCreated {
    pub user_id: String,
    pub username: String,
    pub email: Option<String>,
    pub org_unit_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IamUserDeleted {
    pub user_id: String,
    pub deleted_by: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IamLoginSucceeded {
    pub user_id: String,
    pub ip: String,
    pub user_agent: Option<String>,
    pub method: String,
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
}
