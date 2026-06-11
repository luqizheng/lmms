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

#[derive(Debug, Deserialize)]
pub struct RegisterReq {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegisterResp {
    pub id: u64,
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct SetupStatusResp {
    pub setup_required: bool,
}

#[derive(Debug, Serialize)]
pub struct ErrorResp {
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateOrgUnitReq {
    pub code: String,
    pub name: String,
    pub parent_id: Option<u64>,
    #[serde(default)]
    pub sort: i32,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrgUnitReq {
    pub name: String,
    pub parent_id: Option<u64>,
    pub sort: i32,
    pub status: i8,
}

#[derive(Debug, Deserialize)]
pub struct CreatePositionReq {
    pub code: String,
    pub name: String,
    pub org_unit_id: u64,
    #[serde(default)]
    pub level: i8,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePositionReq {
    pub name: String,
    pub org_unit_id: u64,
    pub level: i8,
    pub status: i8,
}

#[derive(Debug, Deserialize)]
pub struct AssignPositionRolesReq {
    pub role_ids: Vec<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AssignUserPositionsReq {
    pub items: Vec<UserPositionItem>,
}

#[derive(Debug, Deserialize)]
pub struct UserPositionItem {
    pub position_id: u64,
    #[serde(default)]
    pub is_primary: bool,
}
