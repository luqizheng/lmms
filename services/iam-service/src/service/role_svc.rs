use crate::{
    error::IamError,
    state::AppState,
    dto::{CreateRoleRequest, UpdateRoleRequest, RoleQuery, RoleResponse, RoleListResponse},
};
use chrono::Utc;
use common_db::WithTenantScope;
use jwt_ctx::TenantScope;
use nanoid::nanoid;
use sqlx::{FromRow, QueryBuilder, mysql::MySqlQueryResult};
use std::collections::{BTreeSet, HashSet};

pub struct RoleService {
    state: AppState,
}

#[derive(FromRow)]
struct RoleRow {
    id: String,
    name: String,
    code: String,
    description: Option<String>,
    data_scope: String,
    status: String,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    deleted_at: Option<chrono::DateTime<Utc>>,
}

impl RoleService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(&self, req: CreateRoleRequest, scope: &TenantScope) -> Result<RoleResponse, IamError> {
        let role_id = nanoid!();
        let data_scope_str = data_scope_to_str(req.data_scope);
        let tid = scope.tenant_id().unwrap_or(1) as u64;

        sqlx::query(
            r#"INSERT INTO iam_roles (id, tenant_id, name, code, description, data_scope, status, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, 'active', NOW(), NOW())"#,
        )
        .bind(&role_id)
        .bind(tid)
        .bind(&req.name)
        .bind(&req.code)
        .bind(&req.description)
        .bind(data_scope_str)
        .execute(&self.state.db)
        .await?;

        self.get(&role_id, scope).await
    }

    pub async fn get(&self, role_id: &str, scope: &TenantScope) -> Result<RoleResponse, IamError> {
        let mut qb = QueryBuilder::new(
            "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM iam_roles WHERE id = "
        );
        qb.push_bind(role_id);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL");
        let role: Option<RoleRow> = qb.build_query_as::<RoleRow>().fetch_optional(&self.state.db).await?;

        match role {
            Some(r) => Ok(self.to_response(&r)),
            None => Err(IamError::RoleNotFound),
        }
    }

    pub async fn list(&self, query: RoleQuery, scope: &TenantScope) -> Result<RoleListResponse, IamError> {
        let offset = ((query.page - 1) * query.size) as i64;
        let limit = query.size as i64;
        let tid = scope.tenant_id().unwrap_or(1) as u64;

        let roles: Vec<RoleRow> = if let Some(keyword) = &query.keyword {
            let pattern = format!("%{}%", keyword);
            sqlx::query_as(
                "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM iam_roles WHERE deleted_at IS NULL AND tenant_id = ? AND (name LIKE ? OR code LIKE ?) ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(tid)
            .bind(&pattern)
            .bind(&pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.state.db)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM iam_roles WHERE deleted_at IS NULL AND tenant_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(tid)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.state.db)
            .await?
        };

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM iam_roles WHERE deleted_at IS NULL AND tenant_id = ?")
            .bind(tid)
            .fetch_one(&self.state.db)
            .await?;

        Ok(RoleListResponse {
            items: roles.into_iter().map(|r| self.to_response(&r)).collect(),
            total: count.0 as u64,
            page: query.page,
            size: query.size,
        })
    }

    pub async fn update(
        &self,
        role_id: &str,
        req: UpdateRoleRequest,
        actor_id: Option<&str>,
        scope: &TenantScope,
    ) -> Result<RoleResponse, IamError> {
        let mut qb = QueryBuilder::new(
            "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM iam_roles WHERE id = "
        );
        qb.push_bind(role_id);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL");
        let role: Option<RoleRow> = qb.build_query_as::<RoleRow>().fetch_optional(&self.state.db).await?;

        let role = match role {
            Some(r) => r,
            None => return Err(IamError::RoleNotFound),
        };

        let name = req.name.unwrap_or(role.name);
        let description = req.description.or(role.description);
        let data_scope_str = match req.data_scope {
            Some(n) => data_scope_to_str(n).to_string(),
            None => role.data_scope.clone(),
        };
        let status = req.status.unwrap_or(role.status.clone());

        let now_disabled = status == "disabled" && role.status != "disabled";
        let user_ids: Vec<String> = if now_disabled {
            sqlx::query_scalar("SELECT user_id FROM iam_user_roles WHERE role_id = ?")
                .bind(role_id)
                .fetch_all(&self.state.db)
                .await?
        } else {
            Vec::new()
        };

        let mut tx = self.state.db.begin().await?;
        let mut qb = QueryBuilder::new(
            "UPDATE iam_roles SET name = "
        );
        qb.push_bind(&name);
        qb.push(", description = ");
        qb.push_bind(&description);
        qb.push(", data_scope = ");
        qb.push_bind(&data_scope_str);
        qb.push(", status = ");
        qb.push_bind(&status);
        qb.push(", updated_at = NOW() WHERE id = ");
        qb.push_bind(role_id);
        qb.and_tenant(scope);
        qb.build().execute(&mut *tx).await?;

        if now_disabled {
            crate::service::perm_change::enqueue_permission_changed(
                &mut tx,
                self.state.outbox_repo.as_ref(),
                &user_ids,
                "role_disabled",
                actor_id,
            )
            .await?;
        }
        tx.commit().await?;

        if now_disabled {
            crate::service::perm_change::stamp_perm_epochs(&self.state, &user_ids).await;
        }

        self.get(role_id, scope).await
    }

    pub async fn delete(&self, role_id: &str, actor_id: Option<&str>, scope: &TenantScope) -> Result<(), IamError> {
        let user_ids: Vec<String> =
            sqlx::query_scalar("SELECT user_id FROM iam_user_roles WHERE role_id = ?")
                .bind(role_id)
                .fetch_all(&self.state.db)
                .await?;

        let mut tx = self.state.db.begin().await?;
        let mut qb = QueryBuilder::new(
            "UPDATE iam_roles SET deleted_at = NOW(), updated_at = NOW()",
        );
        qb.where_id(role_id);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL");
        let result: MySqlQueryResult = qb.build().execute(&mut *tx).await?;

        if result.rows_affected() == 0 {
            return Err(IamError::RoleNotFound);
        }

        crate::service::perm_change::enqueue_permission_changed(
            &mut tx,
            self.state.outbox_repo.as_ref(),
            &user_ids,
            "role_deleted",
            actor_id,
        )
        .await?;
        tx.commit().await?;

        crate::service::perm_change::stamp_perm_epochs(&self.state, &user_ids).await;
        Ok(())
    }

    pub async fn assign_permissions(
        &self,
        role_id: &str,
        permission_ids: Vec<String>,
        actor_id: Option<&str>,
        scope: &TenantScope,
    ) -> Result<(), IamError> {
        self.get(role_id, scope).await?;

        let user_ids: Vec<String> =
            sqlx::query_scalar("SELECT user_id FROM iam_user_roles WHERE role_id = ?")
                .bind(role_id)
                .fetch_all(&self.state.db)
                .await?;

        let mut tx = self.state.db.begin().await?;
        sqlx::query("DELETE FROM iam_role_permissions WHERE role_id = ?")
            .bind(role_id)
            .execute(&mut *tx)
            .await?;
        for permission_id in &permission_ids {
            sqlx::query(
                "INSERT INTO iam_role_permissions (role_id, permission_id, created_at) VALUES (?, ?, NOW())",
            )
            .bind(role_id)
            .bind(permission_id)
            .execute(&mut *tx)
            .await?;
        }
        crate::service::perm_change::enqueue_permission_changed(
            &mut tx,
            self.state.outbox_repo.as_ref(),
            &user_ids,
            "role_permissions_changed",
            actor_id,
        )
        .await?;
        tx.commit().await?;

        crate::service::perm_change::stamp_perm_epochs(&self.state, &user_ids).await;
        Ok(())
    }

    /// Full-replace a user's role set. Empty `role_ids` is legal (clears).
    /// Duplicate ids in `role_ids` are deduped. Returns the resulting canonical
    /// role_ids (sorted). No-op (new == old after dedup) → returns Ok without
    /// touching the DB, enqueuing an event, or stamping perm_epoch.
    ///
    /// Errors:
    /// - `IamError::UserNotFound` — user_id absent or soft-deleted
    /// - `IamError::InvalidRoleIds(v)` — v lists role_ids not found as active
    pub async fn assign_user_roles(
        &self,
        user_id: &str,
        role_ids: Vec<String>,
        actor_id: Option<&str>,
        scope: &TenantScope,
    ) -> Result<Vec<String>, IamError> {
        let mut tx = self.state.db.begin().await?;

        // 1) user existence + row lock
        let mut qb = QueryBuilder::new("SELECT id FROM iam_users");
        qb.where_id(user_id);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL FOR UPDATE");
        let exists: Option<(String,)> = qb.build_query_as::<(String,)>().fetch_optional(&mut *tx).await?;
        if exists.is_none() {
            return Err(IamError::UserNotFound);
        }

        // 2) dedup + pre-check role_ids (active + not soft-deleted + tenant match)
        let dedup: Vec<String> = {
            let s: BTreeSet<String> = role_ids.into_iter().collect();
            s.into_iter().collect()
        };
        if !dedup.is_empty() {
            let mut qb = QueryBuilder::new("SELECT id FROM iam_roles WHERE id IN (");
            let mut first = true;
            for id in &dedup {
                if !first {
                    qb.push(", ");
                }
                qb.push_bind(id);
                first = false;
            }
            qb.push(") AND status='active' AND deleted_at IS NULL");
            qb.and_tenant(scope);
            let found: Vec<(String,)> = qb.build_query_as::<(String,)>().fetch_all(&mut *tx).await?;
            let found_set: HashSet<&str> = found.iter().map(|(s,)| s.as_str()).collect();
            let invalid: Vec<String> = dedup
                .iter()
                .filter(|id| !found_set.contains(id.as_str()))
                .cloned()
                .collect();
            if !invalid.is_empty() {
                return Err(IamError::InvalidRoleIds(invalid));
            }
        }

        // 3) read old set → diff-skip
        let old: Vec<(String,)> = sqlx::query_as(
            "SELECT role_id FROM iam_user_roles WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_all(&mut *tx)
        .await?;
        let old_set: BTreeSet<String> = old.into_iter().map(|(r,)| r).collect();
        let new_set: BTreeSet<String> = dedup.iter().cloned().collect();
        if old_set == new_set {
            tx.commit().await?;
            return Ok(new_set.into_iter().collect());
        }

        // 4) full replace
        sqlx::query("DELETE FROM iam_user_roles WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        crate::service::user_roles::set_user_roles_in_tx(&mut tx, user_id, &dedup).await?;

        // 5) same-tx outbox event
        crate::service::perm_change::enqueue_permission_changed(
            &mut tx,
            self.state.outbox_repo.as_ref(),
            &[user_id.to_string()],
            "user_roles_changed",
            actor_id,
        )
        .await?;

        tx.commit().await?;

        // 6) post-commit best-effort Redis stamp
        crate::service::perm_change::stamp_perm_epochs(&self.state, &[user_id.to_string()]).await;

        Ok(new_set.into_iter().collect())
    }

    fn to_response(&self, role: &RoleRow) -> RoleResponse {
        RoleResponse {
            id: role.id.clone(),
            name: role.name.clone(),
            code: role.code.clone(),
            description: role.description.clone(),
            data_scope: role.data_scope.clone(),
            status: role.status.clone(),
            created_at: role.created_at,
            updated_at: role.updated_at,
        }
    }
}

/// 数据范围数字 -> 字符串。
/// 与前端 form.vue 的 dataScopeOptions 保持一致：
///   0=self、1=dept、2=dept_and_below、3=all
fn data_scope_to_str(n: i32) -> &'static str {
    match n {
        0 => "self",
        1 => "dept",
        2 => "dept_and_below",
        3 => "all",
        _ => "self",
    }
}
