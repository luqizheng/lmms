use serde::Serialize;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RoleRow {
    pub id: u64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoleDTO {
    pub id: u64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub permission_codes: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl RoleRow {
    pub fn into_dto(self, permission_codes: Vec<String>) -> RoleDTO {
        RoleDTO {
            id: self.id,
            code: self.code,
            name: self.name,
            description: self.description,
            permission_codes,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
