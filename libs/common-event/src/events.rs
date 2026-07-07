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
pub struct IamUserRoleAssigned {
    pub user_id: String,
    pub role_id: String,
    pub tenant_id: i64,
    pub actor_id: Option<String>,
    pub ts: i64,
}

impl Event for IamUserRoleAssigned {
    fn event_type() -> &'static str { "iam.user.role.assigned" }
    fn schema_version() -> u32 { 1 }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IamUserRoleUnassigned {
    pub user_id: String,
    pub role_id: String,
    pub tenant_id: i64,
    pub actor_id: Option<String>,
    pub ts: i64,
}

impl Event for IamUserRoleUnassigned {
    fn event_type() -> &'static str { "iam.user.role.unassigned" }
    fn schema_version() -> u32 { 1 }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IamUserDeptChanged {
    pub user_id: String,
    pub dept_id: Option<String>,
    pub tenant_id: i64,
    pub actor_id: Option<String>,
    pub ts: i64,
}

impl Event for IamUserDeptChanged {
    fn event_type() -> &'static str { "iam.user.dept.changed" }
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationAnnouncementPublished {
    pub announcement_id: i64,
    pub title: String,
    pub pinned: bool,
    pub popover: bool,
    pub user_ids: serde_json::Value,
}

impl Event for NotificationAnnouncementPublished {
    fn event_type() -> &'static str { "notification.announcement.published" }
    fn schema_version() -> u32 { 1 }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IamPermissionChanged {
    pub user_ids: serde_json::Value,
    pub reason: String,
    pub actor_id: Option<String>,
}

impl Event for IamPermissionChanged {
    fn event_type() -> &'static str { "iam.permission.changed" }
    fn schema_version() -> u32 { 1 }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IamTenantCreated {
    pub tenant_id: String,
    pub slug: String,
    pub display_name: String,
    pub admin_user_id: String,
}

impl Event for IamTenantCreated {
    fn event_type() -> &'static str { "iam.tenant.created" }
    fn schema_version() -> u32 { 1 }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileUploaded {
    pub file_id: String,
    pub filename: String,
    pub content_type: Option<String>,
    pub size: i64,
    pub uploader_user_id: String,
}

impl Event for FileUploaded {
    fn event_type() -> &'static str { "file.uploaded" }
    fn schema_version() -> u32 { 1 }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileDeleted {
    pub file_id: String,
    pub deleted_by: String,
}

impl Event for FileDeleted {
    fn event_type() -> &'static str { "file.deleted" }
    fn schema_version() -> u32 { 1 }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigItemChanged {
    pub namespace_id: String,
    pub item_id: String,
    pub key: String,
    pub action: String,
    pub actor_id: Option<String>,
}

impl Event for ConfigItemChanged {
    fn event_type() -> &'static str { "config.item.changed" }
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
    IamUserRoleAssigned(Envelope<IamUserRoleAssigned>),
    IamUserRoleUnassigned(Envelope<IamUserRoleUnassigned>),
    IamUserDeptChanged(Envelope<IamUserDeptChanged>),
    IamLoginSucceeded(Envelope<IamLoginSucceeded>),
    IamLoginFailed(Envelope<IamLoginFailed>),
    NotificationAnnouncementPublished(Envelope<NotificationAnnouncementPublished>),
    IamPermissionChanged(Envelope<IamPermissionChanged>),
    IamTenantCreated(Envelope<IamTenantCreated>),
    FileUploaded(Envelope<FileUploaded>),
    FileDeleted(Envelope<FileDeleted>),
    ConfigItemChanged(Envelope<ConfigItemChanged>),
}
