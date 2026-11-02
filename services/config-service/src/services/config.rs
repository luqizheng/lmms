use crate::error::{AppError, AppErrorExt, AppResult};
use crate::models::config_item::{
    ConfigItem, CreateConfigItem, PagedConfigItems, QueryConfigItems, UpdateConfigItem,
};
use crate::models::namespace::{
    CreateNamespace, Namespace, PagedNamespaces, QueryNamespaces, UpdateNamespace,
};
use crate::models::version::{ConfigVersion, PagedVersions, QueryVersions};
use digua_common_db::dialect::DialectKind;
use digua_common_db::{
    db_execute, db_qb_execute, db_qb_fetch_all, db_qb_fetch_optional, DbPool, WithTenantScope,
};
use digua_common_jwt::JWKSCache;
use digua_jwt_ctx::TenantScope;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct ConfigService {
    pool: DbPool,
    /// Phase 5-B: JWKS cache for verifying inbound user-jwt.
    pub jwks: Arc<JWKSCache>,
}

impl ConfigService {
    pub fn new(pool: DbPool, jwks: Arc<JWKSCache>) -> Self {
        Self { pool, jwks }
    }

    pub fn pool_ref(&self) -> &DbPool {
        &self.pool
    }

    /// 当前方言 —— 只在「标识符转义」这类 SQL 文本处理上需要它。
    ///
    /// 占位符差异由 `db_*!` 宏 / `QueryBuilder::push_bind` 自动处理，业务层不用关心。
    fn dialect(&self) -> DialectKind {
        self.pool.dialect()
    }

    pub async fn create_namespace(
        &self,
        ns: CreateNamespace,
        scope: &TenantScope,
    ) -> AppResult<Namespace> {
        // tenant_id 全链路 i64：sqlx-postgres 无 unsigned 支持（§3 #19）。
        let tenant_id = scope.tenant_id().unwrap_or(1) as i64;
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        db_execute!(
            &self.pool,
            r#"INSERT INTO cfg_namespaces
               (id, tenant_id, app, env, profile, description, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
            &id,
            tenant_id,
            &ns.app,
            &ns.env,
            &ns.profile,
            &ns.description,
            now,
            now
        )?;

        Ok(Namespace {
            id,
            tenant_id,
            app: ns.app,
            env: ns.env,
            profile: ns.profile,
            description: ns.description,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn query_namespaces(
        &self,
        query: QueryNamespaces,
        scope: &TenantScope,
    ) -> AppResult<PagedNamespaces> {
        let page = query.page.unwrap_or(1).max(1);
        let size = query.size.unwrap_or(20).min(100);
        let offset = (page - 1) * size;

        // `size` / `offset` 必须 `as i64`：sqlx-postgres 没有 u32 的 Encode 实现。
        let namespaces: Vec<Namespace> = db_qb_fetch_all!(&self.pool, Namespace, |qb| {
            qb.push("SELECT * FROM cfg_namespaces WHERE 1=1");
            qb.and_tenant(scope);
            qb.push(" ORDER BY created_at DESC LIMIT ")
                .push_bind(size as i64);
            qb.push(" OFFSET ").push_bind(offset as i64);
        })?;
        let total = namespaces.len() as i64;

        Ok(PagedNamespaces {
            items: namespaces,
            total,
            page,
            size,
        })
    }

    pub async fn get_namespace(&self, id: &str, scope: &TenantScope) -> AppResult<Namespace> {
        let ns: Option<Namespace> = db_qb_fetch_optional!(&self.pool, Namespace, |qb| {
            qb.push("SELECT * FROM cfg_namespaces");
            qb.where_id(id);
            qb.and_tenant(scope);
        })?;

        ns.ok_or_else(|| AppError::namespace_not_found(id))
    }

    pub async fn update_namespace(
        &self,
        id: &str,
        update: UpdateNamespace,
        scope: &TenantScope,
    ) -> AppResult<Namespace> {
        let now = chrono::Utc::now();

        db_qb_execute!(&self.pool, |qb| {
            qb.push("UPDATE cfg_namespaces SET description = ");
            qb.push_bind(&update.description);
            qb.push(", updated_at = ");
            qb.push_bind(now);
            qb.where_id(id);
            qb.and_tenant(scope);
        })?;

        self.get_namespace(id, scope).await
    }

    pub async fn delete_namespace(&self, id: &str, scope: &TenantScope) -> AppResult<()> {
        db_qb_execute!(&self.pool, |qb| {
            qb.push("DELETE FROM cfg_namespaces");
            qb.where_id(id);
            qb.and_tenant(scope);
        })?;
        Ok(())
    }

    pub async fn create_config_item(
        &self,
        item: CreateConfigItem,
        scope: &TenantScope,
    ) -> AppResult<ConfigItem> {
        // check-tenant-scope: delegated (namespace ownership enforced by and_tenant in get/verify)
        let exists: Option<(String,)> = db_qb_fetch_optional!(&self.pool, (String,), |qb| {
            qb.push("SELECT id FROM cfg_namespaces");
            qb.where_id(&item.namespace_id);
            qb.and_tenant(scope);
        })?;
        if exists.is_none() {
            return Err(AppError::namespace_not_found(&item.namespace_id));
        }

        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        // `key` 在 MySQL 是保留字（需反引号）、在 PG 需双引号 —— 交给基座层转义，不写方言分支。
        let key_col = self.dialect().escape_identifier("key");
        let sql = format!(
            r#"INSERT INTO cfg_config_items
               (id, namespace_id, {key_col}, value, value_type, is_secret, description, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#
        );
        db_execute!(
            &self.pool,
            sql,
            &id,
            &item.namespace_id,
            &item.key,
            &item.value,
            &item.value_type,
            item.is_secret,
            &item.description,
            now,
            now
        )?;

        Ok(ConfigItem {
            id,
            namespace_id: item.namespace_id,
            key: item.key,
            value: item.value,
            value_type: item.value_type,
            is_secret: item.is_secret,
            description: item.description,
            created_at: now,
            updated_at: now,
            value_masked: false,
        })
    }

    pub async fn query_config_items(
        &self,
        query: QueryConfigItems,
        scope: &TenantScope,
    ) -> AppResult<PagedConfigItems> {
        let page = query.page.unwrap_or(1).max(1);
        let size = query.size.unwrap_or(20).min(100);
        let offset = (page - 1) * size;

        // JOIN cfg_namespaces; hand-written tenant match (and_tenant helper has no alias support).
        // `key` 需方言相关转义（MySQL `` `key` `` / PG `"key"`），故先拼成带别名的完整列名。
        let qualified_key = format!("i.{}", self.dialect().escape_identifier("key"));
        let items: Vec<ConfigItem> = db_qb_fetch_all!(&self.pool, ConfigItem, |qb| {
            qb.push(
                "SELECT i.* FROM cfg_config_items i \
                 JOIN cfg_namespaces n ON i.namespace_id = n.id WHERE 1=1",
            );
            match scope {
                TenantScope::Single(tid) => {
                    qb.push(" AND n.tenant_id = ").push_bind(*tid);
                }
                TenantScope::CrossTenant => {}
            }
            if let Some(ns_id) = &query.namespace_id {
                qb.push(" AND i.namespace_id = ").push_bind(ns_id);
            }
            if let Some(key) = &query.key {
                qb.push(" AND ")
                    .push(&qualified_key)
                    .push(" LIKE ")
                    .push_bind(format!("%{key}%"));
            }
            qb.push(" ORDER BY i.created_at DESC LIMIT ")
                .push_bind(size as i64);
            qb.push(" OFFSET ").push_bind(offset as i64);
        })?;
        let total = items.len() as i64;

        Ok(PagedConfigItems {
            items,
            total,
            page,
            size,
        })
    }

    pub async fn get_config_item(&self, id: &str, scope: &TenantScope) -> AppResult<ConfigItem> {
        // JOIN cfg_namespaces; hand-written tenant match (and_tenant helper has no alias support).
        let item: Option<ConfigItem> = db_qb_fetch_optional!(&self.pool, ConfigItem, |qb| {
            qb.push(
                "SELECT i.* FROM cfg_config_items i \
                 JOIN cfg_namespaces n ON i.namespace_id = n.id WHERE i.id = ",
            );
            qb.push_bind(id);
            match scope {
                TenantScope::Single(tid) => {
                    qb.push(" AND n.tenant_id = ").push_bind(*tid);
                }
                TenantScope::CrossTenant => {}
            }
        })?;

        item.ok_or_else(|| AppError::config_not_found(id))
    }

    pub async fn update_config_item(
        &self,
        id: &str,
        update: UpdateConfigItem,
        scope: &TenantScope,
    ) -> AppResult<ConfigItem> {
        let now = chrono::Utc::now();

        // 设计归属表：MySQL 的多表 UPDATE ... JOIN 在 PG 无等价写法（PG 用 UPDATE ... SET ... FROM ...），
        // 二者无法统一，故显式分支。注意 PG 的 SET 目标不能带别名（SET i.value 非法），
        // 但 SET 表达式里可以带别名（COALESCE(?, i.value) 合法）。
        //
        // ⚠️ 这段 SQL 用 `db_execute!`（静态 SQL + 占位符改写）而**不是** `db_qb_execute!`：
        // `QueryBuilder::push` 是纯文本追加，**不会**把 `?` 改写成 `$N`；`push_bind` 只会在
        // 缓冲区**末尾**追加占位符。把带 `?` 的 head 交给 `push` 会得到
        // `... COALESCE(?, i.value) ... AND i.id = ?$1$2$3...` 这种畸形 SQL。
        let head = match self.dialect() {
            DialectKind::Postgres => {
                "UPDATE cfg_config_items i SET value = COALESCE(?, i.value), \
                 value_type = COALESCE(?, i.value_type), is_secret = COALESCE(?, i.is_secret), \
                 description = COALESCE(?, i.description), updated_at = ? FROM cfg_namespaces n \
                 WHERE i.namespace_id = n.id AND i.id = ?"
            }
            _ => {
                "UPDATE cfg_config_items i JOIN cfg_namespaces n ON i.namespace_id = n.id \
                 SET i.value = COALESCE(?, i.value), i.value_type = COALESCE(?, i.value_type), \
                 i.is_secret = COALESCE(?, i.is_secret), i.description = COALESCE(?, i.description), \
                 i.updated_at = ? WHERE i.id = ?"
            }
        };
        // 两方言 bind 顺序完全一致：(value, value_type, is_secret, description, now, id[, tenant_id])。
        match scope {
            TenantScope::Single(tid) => {
                let sql = format!("{head} AND n.tenant_id = ?");
                db_execute!(
                    &self.pool,
                    sql,
                    &update.value,
                    &update.value_type,
                    update.is_secret,
                    &update.description,
                    now,
                    id,
                    *tid
                )?;
            }
            TenantScope::CrossTenant => {
                db_execute!(
                    &self.pool,
                    head,
                    &update.value,
                    &update.value_type,
                    update.is_secret,
                    &update.description,
                    now,
                    id
                )?;
            }
        }

        self.get_config_item(id, scope).await
    }

    pub async fn delete_config_item(&self, id: &str, scope: &TenantScope) -> AppResult<()> {
        // 同上：MySQL `DELETE i FROM ... JOIN` ↔ PG `DELETE FROM ... USING`，无公共写法。
        // 也用静态 SQL + `db_execute!`，理由见 update_config_item 的注释。
        let head = match self.dialect() {
            DialectKind::Postgres => {
                "DELETE FROM cfg_config_items i USING cfg_namespaces n \
                 WHERE i.namespace_id = n.id AND i.id = ?"
            }
            _ => {
                "DELETE i FROM cfg_config_items i JOIN cfg_namespaces n ON i.namespace_id = n.id \
                 WHERE i.id = ?"
            }
        };
        match scope {
            TenantScope::Single(tid) => {
                let sql = format!("{head} AND n.tenant_id = ?");
                db_execute!(&self.pool, sql, id, *tid)?;
            }
            TenantScope::CrossTenant => {
                db_execute!(&self.pool, head, id)?;
            }
        }
        Ok(())
    }

    pub async fn query_versions(
        &self,
        query: QueryVersions,
        scope: &TenantScope,
    ) -> AppResult<PagedVersions> {
        let page = query.page.unwrap_or(1).max(1);
        let size = query.size.unwrap_or(20).min(100);
        let offset = (page - 1) * size;

        // JOIN cfg_namespaces; hand-written tenant match (and_tenant helper has no alias support).
        let versions: Vec<ConfigVersion> = db_qb_fetch_all!(&self.pool, ConfigVersion, |qb| {
            qb.push(
                "SELECT v.* FROM cfg_config_versions v \
                 JOIN cfg_namespaces n ON v.namespace_id = n.id WHERE 1=1",
            );
            match scope {
                TenantScope::Single(tid) => {
                    qb.push(" AND n.tenant_id = ").push_bind(*tid);
                }
                TenantScope::CrossTenant => {}
            }
            if let Some(ns_id) = &query.namespace_id {
                qb.push(" AND v.namespace_id = ").push_bind(ns_id);
            }
            if let Some(item_id) = &query.item_id {
                qb.push(" AND v.item_id = ").push_bind(item_id);
            }
            qb.push(" ORDER BY v.created_at DESC LIMIT ")
                .push_bind(size as i64);
            qb.push(" OFFSET ").push_bind(offset as i64);
        })?;
        let total = versions.len() as i64;

        Ok(PagedVersions {
            items: versions,
            total,
            page,
            size,
        })
    }
}
