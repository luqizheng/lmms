use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct UpdateMeReq {
    pub email: Option<String>,
    pub phone: Option<String>,
    pub nickname: Option<String>,
    #[serde(default)]
    pub profile: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct PatchExtendsReq {
    pub extends: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoleReq {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleReq {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AssignRolePermissionsReq {
    pub permission_ids: Vec<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AssignUserRolesReq {
    pub role_ids: Vec<u64>,
}

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_size")]
    pub size: u32,
}

fn default_page() -> u32 {
    1
}
fn default_size() -> u32 {
    20
}

#[derive(Debug, Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginSuccessResp {
    pub code: String,
    pub expires_in: u32,
}

#[derive(Debug, Serialize)]
pub struct LockedResp {
    pub error: &'static str,
    pub retry_after: u64,
}

#[derive(Debug, Serialize)]
pub struct InvalidCredsResp {
    pub error: &'static str,
}
