use serde::Serialize;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct PermissionRow {
    pub id: u64,
    pub code: String,
    pub name: String,
    pub resource: String,
    pub action: String,
}
