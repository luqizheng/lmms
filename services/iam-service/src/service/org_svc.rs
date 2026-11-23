use crate::{
    dto::{AssignUsersRequest, CreateOrgUnitRequest, OrgUnitResponse, UpdateOrgUnitRequest},
    error::IamError,
    state::AppState,
};
use chrono::Utc;
use digua_common_db::{db_execute, db_fetch_all, db_fetch_one, WithTenantScope};
use digua_common_event::AnyTx;
use digua_jwt_ctx::TenantScope;
use nanoid::nanoid;
use sqlx::{FromRow, QueryBuilder};

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
    #[allow(dead_code)] // sqlx FromRow needs the column; queries filter on `deleted_at IS NULL`.
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct OrgService {
    state: AppState,
}

impl OrgService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(
        &self,
        req: CreateOrgUnitRequest,
        scope: &TenantScope,
    ) -> Result<OrgUnitResponse, IamError> {
        // tenant_id 统一按有符号 i64 绑定（PG 列即 BIGINT；MySQL UNSIGNED 列存正数无损）。
        let tid = scope.tenant_id().unwrap_or(1);

        // QueryBuilder（and_tenant 条件过滤）→ 双臂（prue 样板）。
        let exists: Option<(String,)> = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => org_code_exists_mysql(p, &req.code, scope).await?,
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => org_code_exists_pg(p, &req.code, scope).await?,
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "org create: dialect feature not enabled".into(),
                ))
            }
        };

        if exists.is_some() {
            return Err(IamError::DuplicateOrgUnit);
        }

        let level = if let Some(parent_id) = &req.parent_id {
            let parent: Option<OrgUnitRow> = match &self.state.db {
                #[cfg(feature = "mysql")]
                digua_common_db::DbPool::Mysql(p) => {
                    fetch_org_unit_mysql(p, parent_id, scope, false).await?
                }
                #[cfg(feature = "postgres")]
                digua_common_db::DbPool::Postgres(p) => {
                    fetch_org_unit_pg(p, parent_id, scope, false).await?
                }
                #[allow(unreachable_patterns)]
                _ => {
                    return Err(IamError::Internal(
                        "org create parent: dialect feature not enabled".into(),
                    ))
                }
            };

            let parent = parent.ok_or(IamError::OrgUnitNotFound)?;
            parent.level + 1
        } else {
            1
        };

        let org_unit_id = nanoid!();

        db_execute!(
            &self.state.db,
            "INSERT INTO iam_org_units (id, parent_id, name, code, level, sort_order, status, tenant_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 'active', ?, NOW(), NOW())",
            &org_unit_id,
            &req.parent_id,
            &req.name,
            &req.code,
            level,
            req.sort_order.unwrap_or(0),
            tid,
        )?;

        // 新建行回读（不过滤软删）→ 双臂。
        let org_unit: Option<OrgUnitRow> = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                fetch_org_unit_mysql(p, &org_unit_id, scope, true).await?
            }
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                fetch_org_unit_pg(p, &org_unit_id, scope, true).await?
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "org create readback: dialect feature not enabled".into(),
                ))
            }
        };
        let org_unit = org_unit.ok_or(IamError::OrgUnitNotFound)?;

        Ok(self.to_response(&org_unit, vec![]))
    }

    // check-tenant-scope: delegated — 租户过滤在 fetch_org_unit_* 的 and_tenant(scope)
    pub async fn get(
        &self,
        org_unit_id: &str,
        scope: &TenantScope,
    ) -> Result<OrgUnitResponse, IamError> {
        let org_unit: Option<OrgUnitRow> = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                fetch_org_unit_mysql(p, org_unit_id, scope, false).await?
            }
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                fetch_org_unit_pg(p, org_unit_id, scope, false).await?
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "org get: dialect feature not enabled".into(),
                ))
            }
        };

        let org_unit = org_unit.ok_or(IamError::OrgUnitNotFound)?;

        let children = self.get_children(org_unit_id, scope).await?;

        Ok(self.to_response(&org_unit, children))
    }

    pub async fn list(&self, scope: &TenantScope) -> Result<Vec<OrgUnitResponse>, IamError> {
        let tid = scope.tenant_id().unwrap_or(1);

        let org_units: Vec<OrgUnitRow> = db_fetch_all!(
            &self.state.db,
            OrgUnitRow,
            "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE parent_id IS NULL AND tenant_id = ? AND deleted_at IS NULL ORDER BY sort_order",
            tid
        )?;

        let mut result = Vec::new();
        for org_unit in org_units {
            let children = self.get_children(&org_unit.id, scope).await?;
            result.push(self.to_response(&org_unit, children));
        }

        Ok(result)
    }

    fn get_children<'a>(
        &'a self,
        parent_id: &'a str,
        scope: &'a TenantScope,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<OrgUnitResponse>, IamError>> + Send + 'a>,
    > {
        Box::pin(async move {
            let children: Vec<OrgUnitRow> = match &self.state.db {
                #[cfg(feature = "mysql")]
                digua_common_db::DbPool::Mysql(p) => {
                    fetch_children_mysql(p, parent_id, scope).await?
                }
                #[cfg(feature = "postgres")]
                digua_common_db::DbPool::Postgres(p) => {
                    fetch_children_pg(p, parent_id, scope).await?
                }
                #[allow(unreachable_patterns)]
                _ => {
                    return Err(IamError::Internal(
                        "org get_children: dialect feature not enabled".into(),
                    ))
                }
            };

            let mut result = Vec::new();
            for child in children {
                let grandchildren = self.get_children(&child.id, scope).await?;
                result.push(self.to_response(&child, grandchildren));
            }

            Ok(result)
        })
    }

    pub async fn update(
        &self,
        org_unit_id: &str,
        req: UpdateOrgUnitRequest,
        scope: &TenantScope,
    ) -> Result<OrgUnitResponse, IamError> {
        let existing: Option<OrgUnitRow> = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                fetch_org_unit_mysql(p, org_unit_id, scope, false).await?
            }
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                fetch_org_unit_pg(p, org_unit_id, scope, false).await?
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "org update: dialect feature not enabled".into(),
                ))
            }
        };

        let existing = existing.ok_or(IamError::OrgUnitNotFound)?;

        let parent_id = req.parent_id.or(existing.parent_id);
        let name = req.name.unwrap_or(existing.name);
        let code = req.code.unwrap_or(existing.code);
        let sort_order = req.sort_order.unwrap_or(existing.sort_order);
        let status = req.status.unwrap_or(existing.status);

        // QueryBuilder UPDATE（and_tenant 条件过滤）→ 双臂类型标注（NOW() 两侧通用）。
        match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                let mut qb =
                    QueryBuilder::<sqlx::MySql>::new("UPDATE iam_org_units SET parent_id = ");
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
                qb.build().execute(p).await?;
            }
            // PG 臂：与 mysql 臂同构（push_bind 自动产出 $1..$N）。
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                let mut qb =
                    QueryBuilder::<sqlx::Postgres>::new("UPDATE iam_org_units SET parent_id = ");
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
                qb.build().execute(p).await?;
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "org update write: dialect feature not enabled".into(),
                ))
            }
        }

        let org_unit: Option<OrgUnitRow> = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                fetch_org_unit_mysql(p, org_unit_id, scope, false).await?
            }
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                fetch_org_unit_pg(p, org_unit_id, scope, false).await?
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "org update readback: dialect feature not enabled".into(),
                ))
            }
        };
        let org_unit = org_unit.ok_or(IamError::OrgUnitNotFound)?;

        Ok(self.to_response(&org_unit, vec![]))
    }

    pub async fn delete(&self, org_unit_id: &str, scope: &TenantScope) -> Result<(), IamError> {
        let tid = scope.tenant_id().unwrap_or(1);

        let (has_children,): (i64,) = db_fetch_one!(
            &self.state.db,
            (i64,),
            "SELECT COUNT(*) FROM iam_org_units WHERE parent_id = ? AND tenant_id = ? AND deleted_at IS NULL",
            org_unit_id,
            tid
        )?;

        if has_children > 0 {
            return Err(IamError::Conflict(
                "Cannot delete org unit with children".to_string(),
            ));
        }

        // 软删 + 解绑用户 + same-tx outbox（dept_changed）→ 双臂（PG 臂 $1）。
        match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                let mut qb = QueryBuilder::<sqlx::MySql>::new(
                    "UPDATE iam_org_units SET deleted_at = NOW(), updated_at = NOW()",
                );
                qb.where_id(org_unit_id);
                qb.and_tenant(scope);
                qb.push(" AND deleted_at IS NULL");
                let result = qb.build().execute(&mut *tx).await?;

                if result.rows_affected() == 0 {
                    return Err(IamError::OrgUnitNotFound);
                }

                let affected_user_ids: Vec<String> = sqlx::query_scalar(
                    "SELECT user_id FROM iam_user_org_units WHERE org_unit_id = ?",
                )
                .bind(org_unit_id)
                .fetch_all(&mut *tx)
                .await?;

                sqlx::query("DELETE FROM iam_user_org_units WHERE org_unit_id = ?")
                    .bind(org_unit_id)
                    .execute(&mut *tx)
                    .await?;

                let ts = Utc::now().timestamp();
                for uid in &affected_user_ids {
                    crate::service::membership_event::enqueue_dept_changed(
                        &mut AnyTx::Mysql(&mut tx),
                        self.state.outbox_repo.as_ref(),
                        uid,
                        None,
                        tid,
                        None,
                        ts,
                    )
                    .await?;
                }

                tx.commit().await?;
            }
            // PG 臂：裸 SQL $1；QueryBuilder 部分与 mysql 同构（push_bind 产出 $N）。
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                let mut qb = QueryBuilder::<sqlx::Postgres>::new(
                    "UPDATE iam_org_units SET deleted_at = NOW(), updated_at = NOW()",
                );
                qb.where_id(org_unit_id);
                qb.and_tenant(scope);
                qb.push(" AND deleted_at IS NULL");
                let result = qb.build().execute(&mut *tx).await?;

                if result.rows_affected() == 0 {
                    return Err(IamError::OrgUnitNotFound);
                }

                let affected_user_ids: Vec<String> = sqlx::query_scalar(
                    "SELECT user_id FROM iam_user_org_units WHERE org_unit_id = $1",
                )
                .bind(org_unit_id)
                .fetch_all(&mut *tx)
                .await?;

                sqlx::query("DELETE FROM iam_user_org_units WHERE org_unit_id = $1")
                    .bind(org_unit_id)
                    .execute(&mut *tx)
                    .await?;

                let ts = Utc::now().timestamp();
                for uid in &affected_user_ids {
                    crate::service::membership_event::enqueue_dept_changed(
                        &mut AnyTx::Postgres(&mut tx),
                        self.state.outbox_repo.as_ref(),
                        uid,
                        None,
                        tid,
                        None,
                        ts,
                    )
                    .await?;
                }

                tx.commit().await?;
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "org delete: dialect feature not enabled".into(),
                ))
            }
        }

        Ok(())
    }

    // check-tenant-scope: delegated
    pub async fn assign_users(
        &self,
        org_unit_id: &str,
        req: AssignUsersRequest,
        scope: &TenantScope,
    ) -> Result<(), IamError> {
        self.get(org_unit_id, scope).await?;
        let tid = scope.tenant_id().unwrap_or(1);

        // 全量替换绑定 + same-tx outbox（dept_changed）→ 双臂（PG 臂 $N；is_primary=false 两侧布尔通用）。
        match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                sqlx::query("DELETE FROM iam_user_org_units WHERE org_unit_id = ?")
                    .bind(org_unit_id)
                    .execute(&mut *tx)
                    .await?;

                let ts = Utc::now().timestamp();
                for user_id in &req.user_ids {
                    sqlx::query(
                        "INSERT INTO iam_user_org_units (user_id, org_unit_id, is_primary, created_at) VALUES (?, ?, ?, NOW())",
                    )
                    .bind(user_id)
                    .bind(org_unit_id)
                    .bind(false)
                    .execute(&mut *tx)
                    .await?;
                    crate::service::membership_event::enqueue_dept_changed(
                        &mut AnyTx::Mysql(&mut tx),
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
            }
            // PG 臂：$1/$2 占位符。
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                sqlx::query("DELETE FROM iam_user_org_units WHERE org_unit_id = $1")
                    .bind(org_unit_id)
                    .execute(&mut *tx)
                    .await?;

                let ts = Utc::now().timestamp();
                for user_id in &req.user_ids {
                    sqlx::query(
                        "INSERT INTO iam_user_org_units (user_id, org_unit_id, is_primary, created_at) \
                         VALUES ($1, $2, false, NOW())",
                    )
                    .bind(user_id)
                    .bind(org_unit_id)
                    .bind(false)
                    .execute(&mut *tx)
                    .await?;
                    crate::service::membership_event::enqueue_dept_changed(
                        &mut AnyTx::Postgres(&mut tx),
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
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "org assign_users: dialect feature not enabled".into(),
                ))
            }
        }

        Ok(())
    }

    fn to_response(
        &self,
        org_unit: &OrgUnitRow,
        children: Vec<OrgUnitResponse>,
    ) -> OrgUnitResponse {
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

/// 双臂共用的 org_unit 单行读取（QueryBuilder + and_tenant；include_deleted=true 跳过软删过滤，用于新建行回读）。
#[cfg(feature = "mysql")]
async fn fetch_org_unit_mysql(
    pool: &sqlx::MySqlPool,
    id: &str,
    scope: &TenantScope,
    include_deleted: bool,
) -> Result<Option<OrgUnitRow>, IamError> {
    let mut qb = QueryBuilder::<sqlx::MySql>::new(
        "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE id = ",
    );
    qb.push_bind(id);
    qb.and_tenant(scope);
    if !include_deleted {
        qb.push(" AND deleted_at IS NULL");
    }
    Ok(qb
        .build_query_as::<OrgUnitRow>()
        .fetch_optional(pool)
        .await?)
}

/// PG 臂：与 mysql 臂同构（QueryBuilder 泛型差异只在类型标注）。
#[cfg(feature = "postgres")]
async fn fetch_org_unit_pg(
    pool: &sqlx::PgPool,
    id: &str,
    scope: &TenantScope,
    include_deleted: bool,
) -> Result<Option<OrgUnitRow>, IamError> {
    let mut qb = QueryBuilder::<sqlx::Postgres>::new(
        "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE id = ",
    );
    qb.push_bind(id);
    qb.and_tenant(scope);
    if !include_deleted {
        qb.push(" AND deleted_at IS NULL");
    }
    Ok(qb
        .build_query_as::<OrgUnitRow>()
        .fetch_optional(pool)
        .await?)
}

/// 双臂共用的 code 重复性检查（QueryBuilder + and_tenant）。
#[cfg(feature = "mysql")]
async fn org_code_exists_mysql(
    pool: &sqlx::MySqlPool,
    code: &str,
    scope: &TenantScope,
) -> Result<Option<(String,)>, IamError> {
    let mut qb = QueryBuilder::<sqlx::MySql>::new("SELECT id FROM iam_org_units WHERE code = ");
    qb.push_bind(code);
    qb.and_tenant(scope);
    qb.push(" AND deleted_at IS NULL");
    Ok(qb
        .build_query_as::<(String,)>()
        .fetch_optional(pool)
        .await?)
}

/// PG 臂：与 mysql 臂同构。
#[cfg(feature = "postgres")]
async fn org_code_exists_pg(
    pool: &sqlx::PgPool,
    code: &str,
    scope: &TenantScope,
) -> Result<Option<(String,)>, IamError> {
    let mut qb = QueryBuilder::<sqlx::Postgres>::new("SELECT id FROM iam_org_units WHERE code = ");
    qb.push_bind(code);
    qb.and_tenant(scope);
    qb.push(" AND deleted_at IS NULL");
    Ok(qb
        .build_query_as::<(String,)>()
        .fetch_optional(pool)
        .await?)
}

/// 双臂共用的直接子节点读取（递归建树用）。
#[cfg(feature = "mysql")]
async fn fetch_children_mysql(
    pool: &sqlx::MySqlPool,
    parent_id: &str,
    scope: &TenantScope,
) -> Result<Vec<OrgUnitRow>, IamError> {
    let mut qb = QueryBuilder::<sqlx::MySql>::new(
        "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE parent_id = ",
    );
    qb.push_bind(parent_id);
    qb.and_tenant(scope);
    qb.push(" AND deleted_at IS NULL ORDER BY sort_order");
    Ok(qb.build_query_as::<OrgUnitRow>().fetch_all(pool).await?)
}

/// PG 臂：与 mysql 臂同构。
#[cfg(feature = "postgres")]
async fn fetch_children_pg(
    pool: &sqlx::PgPool,
    parent_id: &str,
    scope: &TenantScope,
) -> Result<Vec<OrgUnitRow>, IamError> {
    let mut qb = QueryBuilder::<sqlx::Postgres>::new(
        "SELECT id, parent_id, name, code, level, sort_order, status, created_at, updated_at, deleted_at FROM iam_org_units WHERE parent_id = ",
    );
    qb.push_bind(parent_id);
    qb.and_tenant(scope);
    qb.push(" AND deleted_at IS NULL ORDER BY sort_order");
    Ok(qb.build_query_as::<OrgUnitRow>().fetch_all(pool).await?)
}
