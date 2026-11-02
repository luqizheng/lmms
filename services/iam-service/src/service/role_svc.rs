use crate::{
    dto::{CreateRoleRequest, RoleListResponse, RoleQuery, RoleResponse, UpdateRoleRequest},
    error::IamError,
    state::AppState,
};
use chrono::Utc;
use digua_common_db::{db_execute, db_fetch_all, db_fetch_one, WithTenantScope};
use digua_common_event::AnyTx;
use digua_jwt_ctx::TenantScope;
use nanoid::nanoid;
use sqlx::{FromRow, QueryBuilder};
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
    #[allow(dead_code)] // sqlx FromRow needs the column; queries filter on `deleted_at IS NULL`.
    deleted_at: Option<chrono::DateTime<Utc>>,
}

impl RoleService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(
        &self,
        req: CreateRoleRequest,
        scope: &TenantScope,
    ) -> Result<RoleResponse, IamError> {
        let role_id = nanoid!();
        let data_scope_str = data_scope_to_str(req.data_scope);
        // tenant_id 统一按有符号 i64 绑定（PG 列即 BIGINT；MySQL UNSIGNED 列存正数无损）。
        let tid = scope.tenant_id().unwrap_or(1) as i64;

        db_execute!(
            &self.state.db,
            r#"INSERT INTO iam_roles (id, tenant_id, name, code, description, data_scope, status, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, 'active', NOW(), NOW())"#,
            &role_id,
            tid,
            &req.name,
            &req.code,
            &req.description,
            data_scope_str,
        )?;

        self.get(&role_id, scope).await
    }

    pub async fn get(&self, role_id: &str, scope: &TenantScope) -> Result<RoleResponse, IamError> {
        // QueryBuilder 方言泛型（push_bind 产出方言占位符）→ 双臂取池（prue 样板）。
        let role = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => fetch_role_mysql(p, role_id, scope).await?,
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => fetch_role_pg(p, role_id, scope).await?,
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "role get: dialect feature not enabled".into(),
                ))
            }
        };

        match role {
            Some(r) => Ok(self.to_response(&r)),
            None => Err(IamError::RoleNotFound),
        }
    }

    pub async fn list(
        &self,
        query: RoleQuery,
        scope: &TenantScope,
    ) -> Result<RoleListResponse, IamError> {
        let offset = ((query.page - 1) * query.size) as i64;
        let limit = query.size as i64;
        let tid = scope.tenant_id().unwrap_or(1) as i64;

        let roles: Vec<RoleRow> = if let Some(keyword) = &query.keyword {
            let pattern = format!("%{}%", keyword);
            db_fetch_all!(
                &self.state.db,
                RoleRow,
                "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM iam_roles WHERE deleted_at IS NULL AND tenant_id = ? AND (name LIKE ? OR code LIKE ?) ORDER BY created_at DESC LIMIT ? OFFSET ?",
                tid,
                &pattern,
                &pattern,
                limit,
                offset
            )?
        } else {
            db_fetch_all!(
                &self.state.db,
                RoleRow,
                "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM iam_roles WHERE deleted_at IS NULL AND tenant_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
                tid,
                limit,
                offset
            )?
        };

        let (count,): (i64,) = db_fetch_one!(
            &self.state.db,
            (i64,),
            "SELECT COUNT(*) FROM iam_roles WHERE deleted_at IS NULL AND tenant_id = ?",
            tid
        )?;

        Ok(RoleListResponse {
            items: roles.into_iter().map(|r| self.to_response(&r)).collect(),
            total: count as u64,
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
        let role = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => fetch_role_mysql(p, role_id, scope).await?,
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => fetch_role_pg(p, role_id, scope).await?,
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "role update: dialect feature not enabled".into(),
                ))
            }
        };

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
            db_fetch_all!(
                &self.state.db,
                (String,),
                "SELECT user_id FROM iam_user_roles WHERE role_id = ?",
                role_id
            )?
            .into_iter()
            .map(|(u,)| u)
            .collect()
        } else {
            Vec::new()
        };

        let tid = scope.tenant_id().unwrap_or(1) as i64;
        // 事务（QueryBuilder UPDATE + same-tx outbox）→ 双臂。
        match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                let mut qb = QueryBuilder::<sqlx::MySql>::new("UPDATE iam_roles SET name = ");
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
                        &mut AnyTx::Mysql(&mut tx),
                        self.state.outbox_repo.as_ref(),
                        &user_ids,
                        "role_disabled",
                        actor_id,
                    )
                    .await?;
                    let ts = Utc::now().timestamp();
                    for uid in &user_ids {
                        crate::service::membership_event::enqueue_role_unassigned(
                            &mut AnyTx::Mysql(&mut tx),
                            self.state.outbox_repo.as_ref(),
                            uid,
                            role_id,
                            tid,
                            actor_id,
                            ts,
                        )
                        .await?;
                    }
                }
                tx.commit().await?;
            }
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                let mut qb = QueryBuilder::<sqlx::Postgres>::new("UPDATE iam_roles SET name = ");
                qb.push_bind(&name);
                qb.push(", description = ");
                qb.push_bind(&description);
                qb.push(", data_scope = ");
                qb.push_bind(&data_scope_str);
                qb.push(", status = ");
                qb.push_bind(&status);
                // NOW() → now()：PG 列为 TIMESTAMPTZ(3)，语义等价（归属表 #22）。
                qb.push(", updated_at = now() WHERE id = ");
                qb.push_bind(role_id);
                qb.and_tenant(scope);
                qb.build().execute(&mut *tx).await?;
                if now_disabled {
                    crate::service::perm_change::enqueue_permission_changed(
                        &mut AnyTx::Postgres(&mut tx),
                        self.state.outbox_repo.as_ref(),
                        &user_ids,
                        "role_disabled",
                        actor_id,
                    )
                    .await?;
                    let ts = Utc::now().timestamp();
                    for uid in &user_ids {
                        crate::service::membership_event::enqueue_role_unassigned(
                            &mut AnyTx::Postgres(&mut tx),
                            self.state.outbox_repo.as_ref(),
                            uid,
                            role_id,
                            tid,
                            actor_id,
                            ts,
                        )
                        .await?;
                    }
                }
                tx.commit().await?;
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "role update tx: dialect feature not enabled".into(),
                ))
            }
        }

        if now_disabled {
            crate::service::perm_change::stamp_perm_epochs(&self.state, &user_ids).await;
        }

        self.get(role_id, scope).await
    }

    pub async fn delete(
        &self,
        role_id: &str,
        actor_id: Option<&str>,
        scope: &TenantScope,
    ) -> Result<(), IamError> {
        let user_ids: Vec<String> = db_fetch_all!(
            &self.state.db,
            (String,),
            "SELECT user_id FROM iam_user_roles WHERE role_id = ?",
            role_id
        )?
        .into_iter()
        .map(|(u,)| u)
        .collect();

        let tid = scope.tenant_id().unwrap_or(1) as i64;
        let affected = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                let mut qb = QueryBuilder::<sqlx::MySql>::new(
                    "UPDATE iam_roles SET deleted_at = NOW(), updated_at = NOW()",
                );
                qb.where_id(role_id);
                qb.and_tenant(scope);
                qb.push(" AND deleted_at IS NULL");
                let result = qb.build().execute(&mut *tx).await?;
                if result.rows_affected() == 0 {
                    return Err(IamError::RoleNotFound);
                }
                crate::service::perm_change::enqueue_permission_changed(
                    &mut AnyTx::Mysql(&mut tx),
                    self.state.outbox_repo.as_ref(),
                    &user_ids,
                    "role_deleted",
                    actor_id,
                )
                .await?;
                let ts = Utc::now().timestamp();
                for uid in &user_ids {
                    crate::service::membership_event::enqueue_role_unassigned(
                        &mut AnyTx::Mysql(&mut tx),
                        self.state.outbox_repo.as_ref(),
                        uid,
                        role_id,
                        tid,
                        actor_id,
                        ts,
                    )
                    .await?;
                }
                tx.commit().await?;
                result.rows_affected()
            }
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                let mut qb = QueryBuilder::<sqlx::Postgres>::new(
                    "UPDATE iam_roles SET deleted_at = now(), updated_at = now()",
                );
                qb.where_id(role_id);
                qb.and_tenant(scope);
                qb.push(" AND deleted_at IS NULL");
                let result = qb.build().execute(&mut *tx).await?;
                if result.rows_affected() == 0 {
                    return Err(IamError::RoleNotFound);
                }
                crate::service::perm_change::enqueue_permission_changed(
                    &mut AnyTx::Postgres(&mut tx),
                    self.state.outbox_repo.as_ref(),
                    &user_ids,
                    "role_deleted",
                    actor_id,
                )
                .await?;
                let ts = Utc::now().timestamp();
                for uid in &user_ids {
                    crate::service::membership_event::enqueue_role_unassigned(
                        &mut AnyTx::Postgres(&mut tx),
                        self.state.outbox_repo.as_ref(),
                        uid,
                        role_id,
                        tid,
                        actor_id,
                        ts,
                    )
                    .await?;
                }
                tx.commit().await?;
                result.rows_affected()
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "role delete: dialect feature not enabled".into(),
                ))
            }
        };
        let _ = affected;

        crate::service::perm_change::stamp_perm_epochs(&self.state, &user_ids).await;
        Ok(())
    }

    // check-tenant-scope: delegated
    pub async fn assign_permissions(
        &self,
        role_id: &str,
        permission_ids: Vec<String>,
        actor_id: Option<&str>,
        scope: &TenantScope,
    ) -> Result<(), IamError> {
        self.get(role_id, scope).await?;

        let user_ids: Vec<String> = db_fetch_all!(
            &self.state.db,
            (String,),
            "SELECT user_id FROM iam_user_roles WHERE role_id = ?",
            role_id
        )?
        .into_iter()
        .map(|(u,)| u)
        .collect();

        match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
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
                    &mut AnyTx::Mysql(&mut tx),
                    self.state.outbox_repo.as_ref(),
                    &user_ids,
                    "role_permissions_changed",
                    actor_id,
                )
                .await?;
                tx.commit().await?;
            }
            // PG 臂：$N 占位符 + now()（NOW() → now()，TIMESTAMPTZ(3) 列语义等价）。
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                sqlx::query("DELETE FROM iam_role_permissions WHERE role_id = $1")
                    .bind(role_id)
                    .execute(&mut *tx)
                    .await?;
                for permission_id in &permission_ids {
                    sqlx::query(
                        "INSERT INTO iam_role_permissions (role_id, permission_id, created_at) \
                         VALUES ($1, $2, now())",
                    )
                    .bind(role_id)
                    .bind(permission_id)
                    .execute(&mut *tx)
                    .await?;
                }
                crate::service::perm_change::enqueue_permission_changed(
                    &mut AnyTx::Postgres(&mut tx),
                    self.state.outbox_repo.as_ref(),
                    &user_ids,
                    "role_permissions_changed",
                    actor_id,
                )
                .await?;
                tx.commit().await?;
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "assign_permissions: dialect feature not enabled".into(),
                ))
            }
        }

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
        let tid = scope.tenant_id().unwrap_or(1) as i64;

        // 2) dedup + pre-check role_ids（池级读，事务外；双臂仅因 QueryBuilder 泛型）。
        let dedup: Vec<String> = {
            let s: BTreeSet<String> = role_ids.into_iter().collect();
            s.into_iter().collect()
        };
        if !dedup.is_empty() {
            let invalid = match &self.state.db {
                #[cfg(feature = "mysql")]
                digua_common_db::DbPool::Mysql(p) => {
                    let mut qb =
                        QueryBuilder::<sqlx::MySql>::new("SELECT id FROM iam_roles WHERE id IN (");
                    push_id_list(&mut qb, &dedup);
                    qb.push(") AND status='active' AND deleted_at IS NULL");
                    qb.and_tenant(scope);
                    let found: Vec<(String,)> =
                        qb.build_query_as::<(String,)>().fetch_all(p).await?;
                    invalid_ids(&found, &dedup)
                }
                #[cfg(feature = "postgres")]
                digua_common_db::DbPool::Postgres(p) => {
                    let mut qb = QueryBuilder::<sqlx::Postgres>::new(
                        "SELECT id FROM iam_roles WHERE id IN (",
                    );
                    push_id_list_pg(&mut qb, &dedup);
                    qb.push(") AND status='active' AND deleted_at IS NULL");
                    qb.and_tenant(scope);
                    let found: Vec<(String,)> =
                        qb.build_query_as::<(String,)>().fetch_all(p).await?;
                    invalid_ids(&found, &dedup)
                }
                #[allow(unreachable_patterns)]
                _ => {
                    return Err(IamError::Internal(
                        "assign_user_roles: dialect feature not enabled".into(),
                    ))
                }
            };
            if !invalid.is_empty() {
                return Err(IamError::InvalidRoleIds(invalid));
            }
        }

        // 事务主体：user 锁定 → old set diff → 事件 → 全量替换 → outbox → commit。
        match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                // 1) user existence + row lock
                let mut qb = QueryBuilder::<sqlx::MySql>::new("SELECT id FROM iam_users");
                qb.where_id(user_id);
                qb.and_tenant(scope);
                qb.push(" AND deleted_at IS NULL FOR UPDATE");
                let exists: Option<(String,)> = qb
                    .build_query_as::<(String,)>()
                    .fetch_optional(&mut *tx)
                    .await?;
                if exists.is_none() {
                    return Err(IamError::UserNotFound);
                }

                // 3) read old set → diff-skip
                let old: Vec<(String,)> =
                    sqlx::query_as("SELECT role_id FROM iam_user_roles WHERE user_id = ?")
                        .bind(user_id)
                        .fetch_all(&mut *tx)
                        .await?;
                let old_set: BTreeSet<String> = old.into_iter().map(|(r,)| r).collect();
                let new_set: BTreeSet<String> = dedup.iter().cloned().collect();
                if old_set == new_set {
                    tx.commit().await?;
                    return Ok(new_set.into_iter().collect());
                }

                // 4) full replace — emit membership events for diff BEFORE destructive ops
                let to_remove: Vec<String> = old_set.difference(&new_set).cloned().collect();
                let to_add: Vec<String> = new_set.difference(&old_set).cloned().collect();
                let ts = Utc::now().timestamp();
                for rid in &to_remove {
                    crate::service::membership_event::enqueue_role_unassigned(
                        &mut AnyTx::Mysql(&mut tx),
                        self.state.outbox_repo.as_ref(),
                        user_id,
                        rid,
                        tid,
                        actor_id,
                        ts,
                    )
                    .await?;
                }
                sqlx::query("DELETE FROM iam_user_roles WHERE user_id = ?")
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                crate::service::user_roles::set_user_roles_in_tx(
                    &mut AnyTx::Mysql(&mut tx),
                    user_id,
                    &dedup,
                )
                .await?;
                for rid in &to_add {
                    crate::service::membership_event::enqueue_role_assigned(
                        &mut AnyTx::Mysql(&mut tx),
                        self.state.outbox_repo.as_ref(),
                        user_id,
                        rid,
                        tid,
                        actor_id,
                        ts,
                    )
                    .await?;
                }

                // 5) same-tx outbox event
                crate::service::perm_change::enqueue_permission_changed(
                    &mut AnyTx::Mysql(&mut tx),
                    self.state.outbox_repo.as_ref(),
                    &[user_id.to_string()],
                    "user_roles_changed",
                    actor_id,
                )
                .await?;
                tx.commit().await?;
            }
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                let mut qb = QueryBuilder::<sqlx::Postgres>::new("SELECT id FROM iam_users");
                qb.where_id(user_id);
                qb.and_tenant(scope);
                qb.push(" AND deleted_at IS NULL FOR UPDATE");
                let exists: Option<(String,)> = qb
                    .build_query_as::<(String,)>()
                    .fetch_optional(&mut *tx)
                    .await?;
                if exists.is_none() {
                    return Err(IamError::UserNotFound);
                }

                let old: Vec<(String,)> =
                    sqlx::query_as("SELECT role_id FROM iam_user_roles WHERE user_id = $1")
                        .bind(user_id)
                        .fetch_all(&mut *tx)
                        .await?;
                let old_set: BTreeSet<String> = old.into_iter().map(|(r,)| r).collect();
                let new_set: BTreeSet<String> = dedup.iter().cloned().collect();
                if old_set == new_set {
                    tx.commit().await?;
                    return Ok(new_set.into_iter().collect());
                }

                let to_remove: Vec<String> = old_set.difference(&new_set).cloned().collect();
                let to_add: Vec<String> = new_set.difference(&old_set).cloned().collect();
                let ts = Utc::now().timestamp();
                for rid in &to_remove {
                    crate::service::membership_event::enqueue_role_unassigned(
                        &mut AnyTx::Postgres(&mut tx),
                        self.state.outbox_repo.as_ref(),
                        user_id,
                        rid,
                        tid,
                        actor_id,
                        ts,
                    )
                    .await?;
                }
                sqlx::query("DELETE FROM iam_user_roles WHERE user_id = $1")
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                crate::service::user_roles::set_user_roles_in_tx(
                    &mut AnyTx::Postgres(&mut tx),
                    user_id,
                    &dedup,
                )
                .await?;
                for rid in &to_add {
                    crate::service::membership_event::enqueue_role_assigned(
                        &mut AnyTx::Postgres(&mut tx),
                        self.state.outbox_repo.as_ref(),
                        user_id,
                        rid,
                        tid,
                        actor_id,
                        ts,
                    )
                    .await?;
                }

                crate::service::perm_change::enqueue_permission_changed(
                    &mut AnyTx::Postgres(&mut tx),
                    self.state.outbox_repo.as_ref(),
                    &[user_id.to_string()],
                    "user_roles_changed",
                    actor_id,
                )
                .await?;
                tx.commit().await?;
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "assign_user_roles tx: dialect feature not enabled".into(),
                ))
            }
        }

        // 6) post-commit best-effort Redis stamp
        crate::service::perm_change::stamp_perm_epochs(&self.state, &[user_id.to_string()]).await;

        Ok(dedup)
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

/// 双臂共用的角色行读取（QueryBuilder + and_tenant）。
#[cfg(feature = "mysql")]
async fn fetch_role_mysql(
    pool: &sqlx::MySqlPool,
    role_id: &str,
    scope: &TenantScope,
) -> Result<Option<RoleRow>, IamError> {
    let mut qb = QueryBuilder::<sqlx::MySql>::new(
        "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM iam_roles WHERE id = ",
    );
    qb.push_bind(role_id);
    qb.and_tenant(scope);
    qb.push(" AND deleted_at IS NULL");
    Ok(qb.build_query_as::<RoleRow>().fetch_optional(pool).await?)
}

/// PG 臂：与 mysql 臂同构（QueryBuilder 泛型差异只在类型标注）。
#[cfg(feature = "postgres")]
async fn fetch_role_pg(
    pool: &sqlx::PgPool,
    role_id: &str,
    scope: &TenantScope,
) -> Result<Option<RoleRow>, IamError> {
    let mut qb = QueryBuilder::<sqlx::Postgres>::new(
        "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM iam_roles WHERE id = ",
    );
    qb.push_bind(role_id);
    qb.and_tenant(scope);
    qb.push(" AND deleted_at IS NULL");
    Ok(qb.build_query_as::<RoleRow>().fetch_optional(pool).await?)
}

/// 把 id 列表以 bind 方式推进 QueryBuilder（双臂共用；占位符由 QueryBuilder 生成）。
/// `'a` 一致：push_bind 借用的生命周期必须与 QueryBuilder 参数生命周期绑定。
#[cfg(feature = "mysql")]
fn push_id_list<'a>(qb: &mut QueryBuilder<'a, sqlx::MySql>, ids: &'a [String]) {
    let mut first = true;
    for id in ids {
        if !first {
            qb.push(", ");
        }
        qb.push_bind(id);
        first = false;
    }
}

/// PG 版 `push_id_list`（泛型仅差类型参数，不能与 mysql 版共用一个签名）。
#[cfg(feature = "postgres")]
fn push_id_list_pg<'a>(qb: &mut QueryBuilder<'a, sqlx::Postgres>, ids: &'a [String]) {
    let mut first = true;
    for id in ids {
        if !first {
            qb.push(", ");
        }
        qb.push_bind(id);
        first = false;
    }
}

/// 由预检结果算出无效 id 集合（双臂共用）。
fn invalid_ids(found: &[(String,)], dedup: &[String]) -> Vec<String> {
    let found_set: HashSet<&str> = found.iter().map(|(s,)| s.as_str()).collect();
    dedup
        .iter()
        .filter(|id| !found_set.contains(id.as_str()))
        .cloned()
        .collect()
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
