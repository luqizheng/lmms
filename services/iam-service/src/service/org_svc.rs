use crate::{
    error::IamError,
    state::AppState,
    dto::{CreateOrgUnitRequest, UpdateOrgUnitRequest, OrgUnitResponse, AssignUsersRequest},
};
use nanoid::nanoid;
use sqlx::mysql::MySqlQueryResult;
use sqlx::FromRow;

#[derive(FromRow)]
struct OrgUnitRow {
    id: String,
    parent_id: Option<String>,
    name: String,
    code: String,
    level: i32,
    sort_order: i32,
    status: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct OrgService {
    state: AppState,
}

impl OrgService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(&self, req: CreateOrgUnitRequest, tenant_id: i64) -> Result<OrgUnitResponse, IamError> {
        let exists: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM iam_org_units WHERE code = ? AND tenant_id = ? AND deleted_at IS NULL"
        )
        .bind(&req.code)
        .bind(tenant_id)
        .fetch_optional(&self.state.db)
        .await?;

        if exists.is_some() {
            return Err(IamError::DuplicateOrgUnit);
        }

        let level = if let Some(parent_id) = &req.parent_id {
            let parent: Option<OrgUnitRow> = sqlx::query_as(
                "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE id = ? AND tenant_id = ? AND deleted_at IS NULL"
            )
            .bind(parent_id)
            .bind(tenant_id)
            .fetch_optional(&self.state.db)
            .await?;

            let parent = parent.ok_or(IamError::OrgUnitNotFound)?;
            parent.level + 1
        } else {
            1
        };

        let org_unit_id = nanoid!();

        sqlx::query(
            "INSERT INTO iam_org_units (id, parent_id, name, code, level, sort_order, status, tenant_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 'active', ?, NOW(), NOW())"
        )
        .bind(&org_unit_id)
        .bind(&req.parent_id)
        .bind(&req.name)
        .bind(&req.code)
        .bind(level)
        .bind(req.sort_order.unwrap_or(0))
        .bind(tenant_id)
        .execute(&self.state.db)
        .await?;

        let org_unit: OrgUnitRow = sqlx::query_as(
            "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE id = ?"
        )
        .bind(&org_unit_id)
        .fetch_one(&self.state.db)
        .await?;

        Ok(self.to_response(&org_unit, vec![]))
    }

    pub async fn get(&self, org_unit_id: &str, tenant_id: i64) -> Result<OrgUnitResponse, IamError> {
        let org_unit: Option<OrgUnitRow> = sqlx::query_as(
            "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE id = ? AND tenant_id = ? AND deleted_at IS NULL"
        )
        .bind(org_unit_id)
        .bind(tenant_id)
        .fetch_optional(&self.state.db)
        .await?;

        let org_unit = org_unit.ok_or(IamError::OrgUnitNotFound)?;

        let children = self.get_children(&org_unit_id, tenant_id).await?;

        Ok(self.to_response(&org_unit, children))
    }

    pub async fn list(&self, tenant_id: i64) -> Result<Vec<OrgUnitResponse>, IamError> {
        let org_units: Vec<OrgUnitRow> = sqlx::query_as(
            "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE parent_id IS NULL AND tenant_id = ? AND deleted_at IS NULL ORDER BY sort_order"
        )
        .bind(tenant_id)
        .fetch_all(&self.state.db)
        .await?;

        let mut result = Vec::new();
        for org_unit in org_units {
            let children = self.get_children(&org_unit.id, tenant_id).await?;
            result.push(self.to_response(&org_unit, children));
        }

        Ok(result)
    }

    fn get_children<'a>(&'a self, parent_id: &'a str, tenant_id: i64) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<OrgUnitResponse>, IamError>> + Send + 'a>> {
        Box::pin(async move {
            let children: Vec<OrgUnitRow> = sqlx::query_as(
                "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE parent_id = ? AND tenant_id = ? AND deleted_at IS NULL ORDER BY sort_order"
            )
            .bind(parent_id)
            .bind(tenant_id)
            .fetch_all(&self.state.db)
            .await?;

            let mut result = Vec::new();
            for child in children {
                let grandchildren = self.get_children(&child.id, tenant_id).await?;
                result.push(self.to_response(&child, grandchildren));
            }

            Ok(result)
        })
    }

    pub async fn update(&self, org_unit_id: &str, req: UpdateOrgUnitRequest, tenant_id: i64) -> Result<OrgUnitResponse, IamError> {
        let existing: Option<OrgUnitRow> = sqlx::query_as(
            "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE id = ? AND tenant_id = ? AND deleted_at IS NULL"
        )
        .bind(org_unit_id)
        .bind(tenant_id)
        .fetch_optional(&self.state.db)
        .await?;

        let existing = existing.ok_or(IamError::OrgUnitNotFound)?;

        let parent_id = req.parent_id.or(existing.parent_id);
        let name = req.name.unwrap_or(existing.name);
        let code = req.code.unwrap_or(existing.code);
        let sort_order = req.sort_order.unwrap_or(existing.sort_order);
        let status = req.status.unwrap_or(existing.status);

        sqlx::query(
            "UPDATE iam_org_units SET parent_id = ?, name = ?, code = ?, sort_order = ?, status = ?, updated_at = NOW() WHERE id = ? AND tenant_id = ? AND deleted_at IS NULL"
        )
        .bind(&parent_id)
        .bind(&name)
        .bind(&code)
        .bind(sort_order)
        .bind(&status)
        .bind(org_unit_id)
        .bind(tenant_id)
        .execute(&self.state.db)
        .await?;

        let org_unit: OrgUnitRow = sqlx::query_as(
            "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(org_unit_id)
        .fetch_one(&self.state.db)
        .await?;

        Ok(self.to_response(&org_unit, vec![]))
    }

    pub async fn delete(&self, org_unit_id: &str, tenant_id: i64) -> Result<(), IamError> {
        let has_children: (i32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM iam_org_units WHERE parent_id = ? AND tenant_id = ? AND deleted_at IS NULL"
        )
        .bind(org_unit_id)
        .bind(tenant_id)
        .fetch_one(&self.state.db)
        .await?;

        if has_children.0 > 0 {
            return Err(IamError::Conflict("Cannot delete org unit with children".to_string()));
        }

        let result: MySqlQueryResult = sqlx::query(
            "UPDATE iam_org_units SET deleted_at = NOW(), updated_at = NOW() WHERE id = ? AND tenant_id = ? AND deleted_at IS NULL"
        )
        .bind(org_unit_id)
        .bind(tenant_id)
        .execute(&self.state.db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(IamError::OrgUnitNotFound);
        }

        sqlx::query(
            "DELETE FROM iam_user_org_units WHERE org_unit_id = ?"
        )
        .bind(org_unit_id)
        .execute(&self.state.db)
        .await?;

        Ok(())
    }

    pub async fn assign_users(&self, org_unit_id: &str, req: AssignUsersRequest, tenant_id: i64) -> Result<(), IamError> {
        self.get(org_unit_id, tenant_id).await?;

        sqlx::query(
            "DELETE FROM iam_user_org_units WHERE org_unit_id = ?"
        )
        .bind(org_unit_id)
        .execute(&self.state.db)
        .await?;

        for user_id in &req.user_ids {
            sqlx::query(
                "INSERT INTO iam_user_org_units (user_id, org_unit_id, is_primary, created_at) VALUES (?, ?, ?, NOW())"
            )
            .bind(user_id)
            .bind(org_unit_id)
            .bind(false)
            .execute(&self.state.db)
            .await?;
        }

        Ok(())
    }

    fn to_response(&self, org_unit: &OrgUnitRow, children: Vec<OrgUnitResponse>) -> OrgUnitResponse {
        OrgUnitResponse {
            id: org_unit.id.clone(),
            parent_id: org_unit.parent_id.clone(),
            name: org_unit.name.clone(),
            code: org_unit.code.clone(),
            level: org_unit.level,
            sort_order: org_unit.sort_order,
            status: org_unit.status.clone(),
            created_at: org_unit.created_at,
            updated_at: org_unit.updated_at,
            children,
        }
    }
}
