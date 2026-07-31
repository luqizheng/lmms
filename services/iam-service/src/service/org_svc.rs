use crate::{
    error::IamError,
    state::AppState,
    dto::{CreateOrgUnitRequest, UpdateOrgUnitRequest, OrgUnitResponse, AssignUsersRequest},
};
use digua_common_db::WithTenantScope;
use digua_jwt_ctx::TenantScope;
use nanoid::nanoid;
use sqlx::{FromRow, QueryBuilder, mysql::MySqlQueryResult};
use chrono::Utc;

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

    pub async fn create(&self, req: CreateOrgUnitRequest, scope: &TenantScope) -> Result<OrgUnitResponse, IamError> {
        let tid = scope.tenant_id().unwrap_or(1) as u64;

        let mut qb = QueryBuilder::new("SELECT id FROM iam_org_units WHERE code = ");
        qb.push_bind(&req.code);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL");
        let exists: Option<(String,)> = qb.build_query_as::<(String,)>().fetch_optional(&self.state.db).await?;

        if exists.is_some() {
            return Err(IamError::DuplicateOrgUnit);
        }

        let level = if let Some(parent_id) = &req.parent_id {
            let mut qb = QueryBuilder::new(
                "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE id = "
            );
            qb.push_bind(parent_id);
            qb.and_tenant(scope);
            qb.push(" AND deleted_at IS NULL");
            let parent: Option<OrgUnitRow> = qb.build_query_as::<OrgUnitRow>().fetch_optional(&self.state.db).await?;

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
        .bind(tid)
        .execute(&self.state.db)
        .await?;

        let mut qb = QueryBuilder::new(
            "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE id = "
        );
        qb.push_bind(&org_unit_id);
        qb.and_tenant(scope);
        let org_unit: OrgUnitRow = qb.build_query_as::<OrgUnitRow>().fetch_one(&self.state.db).await?;

        Ok(self.to_response(&org_unit, vec![]))
    }

    pub async fn get(&self, org_unit_id: &str, scope: &TenantScope) -> Result<OrgUnitResponse, IamError> {
        let mut qb = QueryBuilder::new(
            "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE id = "
        );
        qb.push_bind(org_unit_id);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL");
        let org_unit: Option<OrgUnitRow> = qb.build_query_as::<OrgUnitRow>().fetch_optional(&self.state.db).await?;

        let org_unit = org_unit.ok_or(IamError::OrgUnitNotFound)?;

        let children = self.get_children(&org_unit_id, scope).await?;

        Ok(self.to_response(&org_unit, children))
    }

    pub async fn list(&self, scope: &TenantScope) -> Result<Vec<OrgUnitResponse>, IamError> {
        let tid = scope.tenant_id().unwrap_or(1) as u64;

        let org_units: Vec<OrgUnitRow> = sqlx::query_as(
            "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE parent_id IS NULL AND tenant_id = ? AND deleted_at IS NULL ORDER BY sort_order"
        )
        .bind(tid)
        .fetch_all(&self.state.db)
        .await?;

        let mut result = Vec::new();
        for org_unit in org_units {
            let children = self.get_children(&org_unit.id, scope).await?;
            result.push(self.to_response(&org_unit, children));
        }

        Ok(result)
    }

    fn get_children<'a>(&'a self, parent_id: &'a str, scope: &'a TenantScope) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<OrgUnitResponse>, IamError>> + Send + 'a>> {
        Box::pin(async move {
            let mut qb = QueryBuilder::new(
                "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE parent_id = "
            );
            qb.push_bind(parent_id);
            qb.and_tenant(scope);
            qb.push(" AND deleted_at IS NULL ORDER BY sort_order");
            let children: Vec<OrgUnitRow> = qb.build_query_as::<OrgUnitRow>().fetch_all(&self.state.db).await?;

            let mut result = Vec::new();
            for child in children {
                let grandchildren = self.get_children(&child.id, scope).await?;
                result.push(self.to_response(&child, grandchildren));
            }

            Ok(result)
        })
    }

    pub async fn update(&self, org_unit_id: &str, req: UpdateOrgUnitRequest, scope: &TenantScope) -> Result<OrgUnitResponse, IamError> {
        let mut qb = QueryBuilder::new(
            "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE id = "
        );
        qb.push_bind(org_unit_id);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL");
        let existing: Option<OrgUnitRow> = qb.build_query_as::<OrgUnitRow>().fetch_optional(&self.state.db).await?;

        let existing = existing.ok_or(IamError::OrgUnitNotFound)?;

        let parent_id = req.parent_id.or(existing.parent_id);
        let name = req.name.unwrap_or(existing.name);
        let code = req.code.unwrap_or(existing.code);
        let sort_order = req.sort_order.unwrap_or(existing.sort_order);
        let status = req.status.unwrap_or(existing.status);

        let mut qb = QueryBuilder::new(
            "UPDATE iam_org_units SET parent_id = "
        );
        qb.push_bind(&parent_id);
        qb.push(", name = ");
        qb.push_bind(&name);
        qb.push(", code = ");
        qb.push_bind(&code);
        qb.push(", sort_order = ");
        qb.push_bind(sort_order);
        qb.push(", status = ");
        qb.push_bind(&status);
        qb.push(", updated_at = NOW() WHERE id = ");
        qb.push_bind(org_unit_id);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL");
        qb.build().execute(&self.state.db).await?;

        let mut qb = QueryBuilder::new(
            "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE id = "
        );
        qb.push_bind(org_unit_id);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL");
        let org_unit: OrgUnitRow = qb.build_query_as::<OrgUnitRow>().fetch_one(&self.state.db).await?;

        Ok(self.to_response(&org_unit, vec![]))
    }

    pub async fn delete(&self, org_unit_id: &str, scope: &TenantScope) -> Result<(), IamError> {
        let tid = scope.tenant_id().unwrap_or(1) as u64;

        let has_children: (i32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM iam_org_units WHERE parent_id = ? AND tenant_id = ? AND deleted_at IS NULL"
        )
        .bind(org_unit_id)
        .bind(tid)
        .fetch_one(&self.state.db)
        .await?;

        if has_children.0 > 0 {
            return Err(IamError::Conflict("Cannot delete org unit with children".to_string()));
        }

        let mut tx = self.state.db.begin().await?;
        let mut qb = QueryBuilder::new(
            "UPDATE iam_org_units SET deleted_at = NOW(), updated_at = NOW()",
        );
        qb.where_id(org_unit_id);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL");
        let result: MySqlQueryResult = qb.build().execute(&mut *tx).await?;

        if result.rows_affected() == 0 {
            return Err(IamError::OrgUnitNotFound);
        }

        let affected_user_ids: Vec<String> = sqlx::query_scalar(
            "SELECT user_id FROM iam_user_org_units WHERE org_unit_id = ?",
        )
        .bind(org_unit_id)
        .fetch_all(&mut *tx)
        .await?;

        sqlx::query(
            "DELETE FROM iam_user_org_units WHERE org_unit_id = ?"
        )
        .bind(org_unit_id)
        .execute(&mut *tx)
        .await?;

        let ts = Utc::now().timestamp();
        for uid in &affected_user_ids {
            crate::service::membership_event::enqueue_dept_changed(
                &mut tx,
                self.state.outbox_repo.as_ref(),
                uid,
                None,
                tid as i64,
                None,
                ts,
            )
            .await?;
        }

        tx.commit().await?;

        Ok(())
    }

    // check-tenant-scope: delegated
    pub async fn assign_users(&self, org_unit_id: &str, req: AssignUsersRequest, scope: &TenantScope) -> Result<(), IamError> {
        self.get(org_unit_id, scope).await?;
        let tid = scope.tenant_id().unwrap_or(1) as i64;

        let mut tx = self.state.db.begin().await?;
        sqlx::query(
            "DELETE FROM iam_user_org_units WHERE org_unit_id = ?"
        )
        .bind(org_unit_id)
        .execute(&mut *tx)
        .await?;

        let ts = Utc::now().timestamp();
        for user_id in &req.user_ids {
            sqlx::query(
                "INSERT INTO iam_user_org_units (user_id, org_unit_id, is_primary, created_at) VALUES (?, ?, ?, NOW())"
            )
            .bind(user_id)
            .bind(org_unit_id)
            .bind(false)
            .execute(&mut *tx)
            .await?;
            crate::service::membership_event::enqueue_dept_changed(
                &mut tx,
                self.state.outbox_repo.as_ref(),
                user_id,
                Some(org_unit_id),
                tid,
                None,
                ts,
            )
            .await?;
        }

        tx.commit().await?;

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