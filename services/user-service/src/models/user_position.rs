use serde::Serialize;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserPositionRow {
    pub user_id: u64,
    pub position_id: u64,
    pub is_primary: i8,
    pub assigned_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserPositionDTO {
    pub position_id: u64,
    pub position_code: String,
    pub position_name: String,
    pub org_unit_id: u64,
    pub org_unit_name: String,
    pub is_primary: bool,
    pub assigned_at: String,
}
