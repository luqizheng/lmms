use crate::{
    dto::{
        CreatePermissionRequest, PermissionListResponse, PermissionQuery, PermissionResponse,
        UpdatePermissionRequest,
    },
    error::IamError,
    state::AppState,
};
use chrono::Utc;
use digua_common_db::{db_fetch_all, db_fetch_one, db_fetch_optional};
use digua_common_event::AnyTx;
use nanoid::nanoid;
use sqlx::FromRow;

pub struct PermissionService {
    state: AppState,
}

#[derive(FromRow)]
struct PermissionRow {
    id: String,
    code: String,
    name: String,
    description: Option<String>,
    category: String,
    resource: Option<String>,
    action: Option<String>,
    sort_order: i32,
    status: String,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    #[allow(dead_code)] // sqlx FromRow needs the column; queries filter on `deleted_at IS NULL`.
    deleted_at: Option<chrono::DateTime<Utc>>,
}

impl PermissionService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(
        &self,
        req: CreatePermissionRequest,
        _actor_id: Option<&str>,
    ) -> Result<PermissionResponse, IamError> {
        let permission_id = nanoid!();
        let category = match req.category {
            0 => "menu",
            1 => "button",
            2 => "api",
            _ => "api",
        };

        // INSERT + 同事务回读 → 双臂（prue 样板；NOW() 两侧通用）。
        let permission: PermissionRow = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                sqlx::query(
                    "INSERT INTO iam_permissions (id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'enabled', NOW(), NOW())",
                )
                .bind(&permission_id)
                .bind(&req.code)
                .bind(&req.name)
                .bind(&req.description)
                .bind(category)
                .bind(&req.resource)
                .bind(&req.action)
                .bind(req.sort_order.unwrap_or(0))
                .execute(&mut *tx)
                .await?;

                let row: PermissionRow = sqlx::query_as(
                    "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE id = ?",
                )
                .bind(&permission_id)
                .fetch_one(&mut *tx)
                .await?;

                tx.commit().await?;
                row
            }
            // PG 臂：$N 占位符。
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                sqlx::query(
                    "INSERT INTO iam_permissions (id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'enabled', NOW(), NOW())",
                )
                .bind(&permission_id)
                .bind(&req.code)
                .bind(&req.name)
                .bind(&req.description)
                .bind(category)
                .bind(&req.resource)
                .bind(&req.action)
                .bind(req.sort_order.unwrap_or(0))
                .execute(&mut *tx)
                .await?;

                let row: PermissionRow = sqlx::query_as(
                    "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE id = $1",
                )
                .bind(&permission_id)
                .fetch_one(&mut *tx)
                .await?;

                tx.commit().await?;
                row
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "permission create: dialect feature not enabled".into(),
                ))
            }
        };

        Ok(self.to_response(&permission))
    }

    pub async fn get(&self, permission_id: &str) -> Result<PermissionResponse, IamError> {
        let permission: Option<PermissionRow> = db_fetch_optional!(
            &self.state.db,
            PermissionRow,
            "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE id = ? AND deleted_at IS NULL",
            permission_id
        )?;

        match permission {
            Some(p) => Ok(self.to_response(&p)),
            None => Err(IamError::PermissionNotFound),
        }
    }

    pub async fn list(&self, query: PermissionQuery) -> Result<PermissionListResponse, IamError> {
        let offset = ((query.page - 1) * query.size) as i64;
        let limit = query.size as i64;

        let permissions: Vec<PermissionRow> = if let Some(keyword) = &query.keyword {
            let pattern = format!("%{}%", keyword);
            if let Some(category) = query.category {
                let cat_str = match category {
                    0 => "menu",
                    1 => "button",
                    2 => "api",
                    _ => "api",
                };
                db_fetch_all!(
                    &self.state.db,
                    PermissionRow,
                    "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE deleted_at IS NULL AND (code LIKE ? OR name LIKE ?) AND category = ? ORDER BY sort_order, created_at LIMIT ? OFFSET ?",
                    &pattern,
                    &pattern,
                    cat_str,
                    limit,
                    offset
                )?
            } else {
                db_fetch_all!(
                    &self.state.db,
                    PermissionRow,
                    "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE deleted_at IS NULL AND (code LIKE ? OR name LIKE ?) ORDER BY sort_order, created_at LIMIT ? OFFSET ?",
                    &pattern,
                    &pattern,
                    limit,
                    offset
                )?
            }
        } else if let Some(category) = query.category {
            let cat_str = match category {
                0 => "menu",
                1 => "button",
                2 => "api",
                _ => "api",
            };
            db_fetch_all!(
                &self.state.db,
                PermissionRow,
                "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE deleted_at IS NULL AND category = ? ORDER BY sort_order, created_at LIMIT ? OFFSET ?",
                cat_str,
                limit,
                offset
            )?
        } else {
            db_fetch_all!(
                &self.state.db,
                PermissionRow,
                "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE deleted_at IS NULL ORDER BY sort_order, created_at LIMIT ? OFFSET ?",
                limit,
                offset
            )?
        };

        let (total,): (i64,) = db_fetch_one!(
            &self.state.db,
            (i64,),
            "SELECT COUNT(*) FROM iam_permissions WHERE deleted_at IS NULL"
        )?;

        Ok(PermissionListResponse {
            items: permissions
                .into_iter()
                .map(|p| self.to_response(&p))
                .collect(),
            total: total as u64,
            page: query.page,
            size: query.size,
        })
    }

    pub async fn update(
        &self,
        permission_id: &str,
        req: UpdatePermissionRequest,
        actor_id: Option<&str>,
    ) -> Result<PermissionResponse, IamError> {
        let permission: Option<PermissionRow> = db_fetch_optional!(
            &self.state.db,
            PermissionRow,
            "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE id = ? AND deleted_at IS NULL",
            permission_id
        )?;

        let permission = match permission {
            Some(p) => p,
            None => return Err(IamError::PermissionNotFound),
        };

        let name = req.name.unwrap_or(permission.name);
        let description = req.description.or(permission.description);
        let sort_order = req.sort_order.unwrap_or(permission.sort_order);
        let status = req.status.unwrap_or(permission.status.clone());

        // Fire iam.permission.changed only on a non-disabled → disabled
        // status transition (not on rename / other updates).
        let now_disabled = status == "disabled" && permission.status != "disabled";
        let user_ids: Vec<String> = if now_disabled {
            db_fetch_all!(
                &self.state.db,
                (String,),
                "SELECT DISTINCT ur.user_id FROM iam_user_roles ur JOIN iam_role_permissions rp ON ur.role_id = rp.role_id WHERE rp.permission_id = ?",
                permission_id
            )?
            .into_iter()
            .map(|(u,)| u)
            .collect()
        } else {
            Vec::new()
        };

        // UPDATE + 同事务回读 + same-tx outbox → 双臂（PG 臂 $N 占位符）。
        let result: PermissionRow = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                sqlx::query(
                    "UPDATE iam_permissions SET name = ?, description = ?, sort_order = ?, status = ?, updated_at = NOW() WHERE id = ?",
                )
                .bind(&name)
                .bind(&description)
                .bind(sort_order)
                .bind(&status)
                .bind(permission_id)
                .execute(&mut *tx)
                .await?;
                let row: PermissionRow = sqlx::query_as(
                    "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE id = ?",
                )
                .bind(permission_id)
                .fetch_one(&mut *tx)
                .await?;

                if now_disabled {
                    crate::service::perm_change::enqueue_permission_changed(
                        &mut AnyTx::Mysql(&mut tx),
                        self.state.outbox_repo.as_ref(),
                        &user_ids,
                        "permission_disabled",
                        actor_id,
                    )
                    .await?;
                }
                tx.commit().await?;
                row
            }
            // PG 臂：$1..$5 占位符。
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                sqlx::query(
                    "UPDATE iam_permissions SET name = $1, description = $2, sort_order = $3, status = $4, updated_at = NOW() WHERE id = $5",
                )
                .bind(&name)
                .bind(&description)
                .bind(sort_order)
                .bind(&status)
                .bind(permission_id)
                .execute(&mut *tx)
                .await?;
                let row: PermissionRow = sqlx::query_as(
                    "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE id = $1",
                )
                .bind(permission_id)
                .fetch_one(&mut *tx)
                .await?;

                if now_disabled {
                    crate::service::perm_change::enqueue_permission_changed(
                        &mut AnyTx::Postgres(&mut tx),
                        self.state.outbox_repo.as_ref(),
                        &user_ids,
                        "permission_disabled",
                        actor_id,
                    )
                    .await?;
                }
                tx.commit().await?;
                row
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "permission update: dialect feature not enabled".into(),
                ))
            }
        };

        if now_disabled {
            crate::service::perm_change::stamp_perm_epochs(&self.state, &user_ids).await;
        }

        Ok(self.to_response(&result))
    }

    pub async fn delete(
        &self,
        permission_id: &str,
        actor_id: Option<&str>,
    ) -> Result<(), IamError> {
        // Expand affected users via the join BEFORE we delete the
        // iam_role_permissions rows we are joining on.
        let user_ids: Vec<String> = db_fetch_all!(
            &self.state.db,
            (String,),
            "SELECT DISTINCT ur.user_id FROM iam_user_roles ur JOIN iam_role_permissions rp ON ur.role_id = rp.role_id WHERE rp.permission_id = ?",
            permission_id
        )?
        .into_iter()
        .map(|(u,)| u)
        .collect();

        // 软删 + 级联解绑 + same-tx outbox → 双臂（PG 臂 $1 占位符）。
        match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                let result = sqlx::query(
                    "UPDATE iam_permissions SET deleted_at = NOW(), updated_at = NOW() WHERE id = ? AND deleted_at IS NULL",
                )
                .bind(permission_id)
                .execute(&mut *tx)
                .await?;

                if result.rows_affected() == 0 {
                    return Err(IamError::PermissionNotFound);
                }

                sqlx::query("DELETE FROM iam_role_permissions WHERE permission_id = ?")
                    .bind(permission_id)
                    .execute(&mut *tx)
                    .await?;

                crate::service::perm_change::enqueue_permission_changed(
                    &mut AnyTx::Mysql(&mut tx),
                    self.state.outbox_repo.as_ref(),
                    &user_ids,
                    "permission_deleted",
                    actor_id,
                )
                .await?;
                tx.commit().await?;
            }
            // PG 臂：$1 占位符。
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                let result = sqlx::query(
                    "UPDATE iam_permissions SET deleted_at = NOW(), updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
                )
                .bind(permission_id)
                .execute(&mut *tx)
                .await?;

                if result.rows_affected() == 0 {
                    return Err(IamError::PermissionNotFound);
                }

                sqlx::query("DELETE FROM iam_role_permissions WHERE permission_id = $1")
                    .bind(permission_id)
                    .execute(&mut *tx)
                    .await?;

                crate::service::perm_change::enqueue_permission_changed(
                    &mut AnyTx::Postgres(&mut tx),
                    self.state.outbox_repo.as_ref(),
                    &user_ids,
                    "permission_deleted",
                    actor_id,
                )
                .await?;
                tx.commit().await?;
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "permission delete: dialect feature not enabled".into(),
                ))
            }
        }

        crate::service::perm_change::stamp_perm_epochs(&self.state, &user_ids).await;
        Ok(())
    }

    fn to_response(&self, permission: &PermissionRow) -> PermissionResponse {
        PermissionResponse {
            id: permission.id.clone(),
            code: permission.code.clone(),
            name: permission.name.clone(),
            description: permission.description.clone(),
            category: permission.category.clone(),
            resource: permission.resource.clone(),
            action: permission.action.clone(),
            sort_order: permission.sort_order,
            status: permission.status.clone(),
            created_at: permission.created_at,
            updated_at: permission.updated_at,
        }
    }
}
