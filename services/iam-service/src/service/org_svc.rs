use crate::{
    error::IamError,
    models::{OrgUnit, UserOrgUnit},
    state::AppState,
    dto::{CreateOrgUnitRequest, UpdateOrgUnitRequest, OrgUnitResponse, AssignUsersRequest},
};
use chrono::Utc;
use nanoid::nanoid;
use sqlx::mysql::MySqlQueryResult;

pub struct OrgService {
    state: AppState,
}

impl OrgService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(&self, req: CreateOrgUnitRequest) -> Result<OrgUnitResponse, IamError> {
        let exists = sqlx::query!(
            "SELECT id FROM org_units WHERE code = ? AND deleted_at IS NULL",
            req.code
        )
        .fetch_optional(&self.state.db)
        .await?
        .is_some();

        if exists {
            return Err(IamError::DuplicateOrgUnit);
        }

        let org_unit_id = nanoid!();
        let level = if req.parent_id.is_some() {
            let parent = sqlx::query_as!(
                OrgUnit,
                "SELECT level FROM org_units WHERE id = ? AND deleted_at IS NULL",
                req.parent_id.as_ref().unwrap()
            )
            .fetch_optional(&self.state.db)
            .await?
            .ok_or(IamError::OrgUnitNotFound)?;
            parent.level + 1
        } else {
            1
        };

        let org_unit = sqlx::query_as!(
            OrgUnit,
            "INSERT INTO org_units (id, parent_id, name, code, level, sort_order, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 'active', ?, ?) RETURNING *",
            org_unit_id,
            req.parent_id,
            req.name,
            req.code,
            level,
            req.sort_order.unwrap_or(0),
            Utc::now(),
            Utc::now()
        )
        .fetch_one(&self.state.db)
        .await?;

        Ok(self.to_response(&org_unit, vec![]))
    }

    pub async fn get(&self, org_unit_id: &str) -> Result<OrgUnitResponse, IamError> {
        let org_unit = sqlx::query_as!(
            OrgUnit,
            "SELECT * FROM org_units WHERE id = ? AND deleted_at IS NULL",
            org_unit_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::OrgUnitNotFound)?;

        let children = self.get_children(&org_unit_id).await?;

        Ok(self.to_response(&org_unit, children))
    }

    pub async fn list(&self) -> Result<Vec<OrgUnitResponse>, IamError> {
        let org_units = sqlx::query_as!(
            OrgUnit,
            "SELECT * FROM org_units WHERE parent_id IS NULL AND deleted_at IS NULL ORDER BY sort_order"
        )
        .fetch_all(&self.state.db)
        .await?;

        let mut result = Vec::new();
        for org_unit in org_units {
            let children = self.get_children(&org_unit.id).await?;
            result.push(self.to_response(&org_unit, children));
        }

        Ok(result)
    }

    async fn get_children(&self, parent_id: &str) -> Result<Vec<OrgUnitResponse>, IamError> {
        let children = sqlx::query_as!(
            OrgUnit,
            "SELECT * FROM org_units WHERE parent_id = ? AND deleted_at IS NULL ORDER BY sort_order",
            parent_id
        )
        .fetch_all(&self.state.db)
        .await?;

        let mut result = Vec::new();
        for child in children {
            let grandchildren = self.get_children(&child.id).await?;
            result.push(self.to_response(&child, grandchildren));
        }

        Ok(result)
    }

    pub async fn update(&self, org_unit_id: &str, req: UpdateOrgUnitRequest) -> Result<OrgUnitResponse, IamError> {
        let existing = sqlx::query_as!(
            OrgUnit,
            "SELECT * FROM org_units WHERE id = ? AND deleted_at IS NULL",
            org_unit_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::OrgUnitNotFound)?;

        let parent_id = req.parent_id.or(existing.parent_id);
        let name = req.name.unwrap_or(existing.name);
        let code = req.code.unwrap_or(existing.code);
        let sort_order = req.sort_order.unwrap_or(existing.sort_order);
        let status = req.status.unwrap_or_else(|| format!("{}", existing.status));

        let org_unit: OrgUnit = sqlx::query_as(
            "UPDATE org_units SET parent_id = ?, name = ?, code = ?, sort_order = ?, status = ?, updated_at = NOW() WHERE id = ? AND deleted_at IS NULL RETURNING *",
        )
        .bind(&parent_id)
        .bind(&name)
        .bind(&code)
        .bind(sort_order)
        .bind(&status)
        .bind(org_unit_id)
        .fetch_one(&self.state.db)
        .await?;

        Ok(self.to_response(&org_unit, vec![]))
    }

    pub async fn delete(&self, org_unit_id: &str) -> Result<(), IamError> {
        let has_children: (i32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM org_units WHERE parent_id = ? AND deleted_at IS NULL",
        )
        .bind(org_unit_id)
        .fetch_one(&self.state.db)
        .await?;

        if has_children.0 > 0 {
            return Err(IamError::Conflict("Cannot delete org unit with children".to_string()));
        }

        let result: MySqlQueryResult = sqlx::query(
            "UPDATE org_units SET deleted_at = NOW(), updated_at = NOW() WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(org_unit_id)
        .execute(&self.state.db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(IamError::OrgUnitNotFound);
        }

        sqlx::query!("DELETE FROM user_org_units WHERE org_unit_id = ?")
            .bind(org_unit_id)
            .execute(&self.state.db)
            .await?;

        Ok(())
    }

    pub async fn assign_users(&self, org_unit_id: &str, req: AssignUsersRequest) -> Result<(), IamError> {
        sqlx::query!(
            "DELETE FROM user_org_units WHERE org_unit_id = ?",
            org_unit_id
        )
        .execute(&self.state.db)
        .await?;

        for user_id in req.user_ids {
            sqlx::query!(
                "INSERT INTO user_org_units (user_id, org_unit_id, is_primary, created_at) VALUES (?, ?, ?, ?)",
                user_id,
                org_unit_id,
                false,
                Utc::now()
            )
            .execute(&self.state.db)
            .await?;
        }

        Ok(())
    }

    fn to_response(&self, org_unit: &OrgUnit, children: Vec<OrgUnitResponse>) -> OrgUnitResponse {
        OrgUnitResponse {
            id: org_unit.id.clone(),
            parent_id: org_unit.parent_id.clone(),
            name: org_unit.name.clone(),
            code: org_unit.code.clone(),
            level: org_unit.level,
            sort_order: org_unit.sort_order,
            status: format!("{}", org_unit.status),
            created_at: org_unit.created_at,
            updated_at: org_unit.updated_at,
            children,
        }
    }
}
