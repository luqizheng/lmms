use serde::Serialize;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PositionRow {
    pub id: u64,
    pub code: String,
    pub name: String,
    pub org_unit_id: u64,
    pub level: i8,
    pub status: i8,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PositionDTO {
    pub id: u64,
    pub code: String,
    pub name: String,
    pub org_unit_id: u64,
    pub org_unit_code: String,
    pub level: i8,
    pub status: i8,
    pub role_codes: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl PositionRow {
    pub fn into_dto(self, org_unit_code: String, role_codes: Vec<String>) -> PositionDTO {
        PositionDTO {
            id: self.id,
            code: self.code,
            name: self.name,
            org_unit_id: self.org_unit_id,
            org_unit_code,
            level: self.level,
            status: self.status,
            role_codes,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
