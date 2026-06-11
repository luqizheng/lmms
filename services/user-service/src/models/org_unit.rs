use serde::Serialize;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrgUnitRow {
    pub id: u64,
    pub code: String,
    pub name: String,
    pub parent_id: Option<u64>,
    pub sort: i32,
    pub status: i8,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrgUnitDTO {
    pub id: u64,
    pub code: String,
    pub name: String,
    pub parent_id: Option<u64>,
    pub sort: i32,
    pub status: i8,
    pub created_at: String,
    pub updated_at: String,
}

impl OrgUnitRow {
    pub fn into_dto(self) -> OrgUnitDTO {
        OrgUnitDTO {
            id: self.id,
            code: self.code,
            name: self.name,
            parent_id: self.parent_id,
            sort: self.sort,
            status: self.status,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
