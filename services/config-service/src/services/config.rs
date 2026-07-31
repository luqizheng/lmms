use crate::error::{AppError, AppErrorExt, AppResult};
use crate::models::config_item::{ConfigItem, CreateConfigItem, PagedConfigItems, QueryConfigItems, UpdateConfigItem};
use crate::models::namespace::{CreateNamespace, Namespace, PagedNamespaces, QueryNamespaces, UpdateNamespace};
use crate::models::version::{ConfigVersion, PagedVersions, QueryVersions};
use digua_common_db::WithTenantScope;
use digua_common_jwt::JWKSCache;
use digua_jwt_ctx::TenantScope;
use sqlx::{MySqlPool, QueryBuilder};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct ConfigService {
    pool: MySqlPool,
    /// Phase 5-B: JWKS cache for verifying inbound user-jwt.
    pub jwks: Arc<JWKSCache>,
}

impl ConfigService {
    pub fn new(pool: MySqlPool, jwks: Arc<JWKSCache>) -> Self {
        Self { pool, jwks }
    }

    pub fn pool_ref(&self) -> &MySqlPool { &self.pool }

    pub async fn create_namespace(&self, ns: CreateNamespace, scope: &TenantScope) -> AppResult<Namespace> {
        let tenant_id = scope.tenant_id().unwrap_or(1) as u64;
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"INSERT INTO cfg_namespaces
               (id, tenant_id, app, env, profile, description, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&id)
        .bind(tenant_id)
        .bind(&ns.app)
        .bind(&ns.env)
        .bind(&ns.profile)
        .bind(&ns.description)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

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

    pub async fn query_namespaces(&self, query: QueryNamespaces, scope: &TenantScope) -> AppResult<PagedNamespaces> {
        let page = query.page.unwrap_or(1).max(1);
        let size = query.size.unwrap_or(20).min(100);
        let offset = (page - 1) * size;

        let mut qb = QueryBuilder::new("SELECT * FROM cfg_namespaces WHERE 1=1");
        qb.and_tenant(scope);
        qb.push(" ORDER BY created_at DESC LIMIT ");
        qb.push_bind(size);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        let namespaces: Vec<Namespace> = qb.build_query_as::<Namespace>().fetch_all(&self.pool).await?;
        let total = namespaces.len() as i64;

        Ok(PagedNamespaces { items: namespaces, total, page, size })
    }

    pub async fn get_namespace(&self, id: &str, scope: &TenantScope) -> AppResult<Namespace> {
        let mut qb = QueryBuilder::new("SELECT * FROM cfg_namespaces");
        qb.where_id(id);
        qb.and_tenant(scope);

        let ns: Option<Namespace> = qb.build_query_as::<Namespace>().fetch_optional(&self.pool).await?;

        ns.ok_or_else(|| AppError::namespace_not_found(id))
    }

    pub async fn update_namespace(&self, id: &str, update: UpdateNamespace, scope: &TenantScope) -> AppResult<Namespace> {
        let now = chrono::Utc::now();

        let mut qb = QueryBuilder::new("UPDATE cfg_namespaces SET description = ");
        qb.push_bind(&update.description);
        qb.push(", updated_at = ");
        qb.push_bind(now);
        qb.where_id(id);
        qb.and_tenant(scope);
        qb.build().execute(&self.pool).await?;

        self.get_namespace(id, scope).await
    }

    pub async fn delete_namespace(&self, id: &str, scope: &TenantScope) -> AppResult<()> {
        let mut qb = QueryBuilder::new("DELETE FROM cfg_namespaces");
        qb.where_id(id);
        qb.and_tenant(scope);
        qb.build().execute(&self.pool).await?;
        Ok(())
    }

    pub async fn create_config_item(&self, item: CreateConfigItem, scope: &TenantScope) -> AppResult<ConfigItem> {
        // check-tenant-scope: delegated (namespace ownership enforced by and_tenant in get/verify)
        let mut qb = QueryBuilder::new("SELECT id FROM cfg_namespaces");
        qb.where_id(&item.namespace_id);
        qb.and_tenant(scope);
        let exists: Option<(String,)> = qb.build_query_as::<(String,)>().fetch_optional(&self.pool).await?;
        if exists.is_none() {
            return Err(AppError::namespace_not_found(&item.namespace_id));
        }

        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"INSERT INTO cfg_config_items
               (id, namespace_id, `key`, value, value_type, is_secret, description, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&id)
        .bind(&item.namespace_id)
        .bind(&item.key)
        .bind(&item.value)
        .bind(&item.value_type)
        .bind(item.is_secret)
        .bind(&item.description)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

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

    pub async fn query_config_items(&self, query: QueryConfigItems, scope: &TenantScope) -> AppResult<PagedConfigItems> {
        let page = query.page.unwrap_or(1).max(1);
        let size = query.size.unwrap_or(20).min(100);
        let offset = (page - 1) * size;

        // JOIN cfg_namespaces; hand-written tenant match (and_tenant helper has no alias support).
        let mut qb = QueryBuilder::new(
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
            qb.push(" AND i.`key` LIKE ").push_bind(format!("%{key}%"));
        }
        qb.push(" ORDER BY i.created_at DESC LIMIT ");
        qb.push_bind(size);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        let items: Vec<ConfigItem> = qb.build_query_as::<ConfigItem>().fetch_all(&self.pool).await?;
        let total = items.len() as i64;

        Ok(PagedConfigItems { items, total, page, size })
    }

    pub async fn get_config_item(&self, id: &str, scope: &TenantScope) -> AppResult<ConfigItem> {
        // JOIN cfg_namespaces; hand-written tenant match (and_tenant helper has no alias support).
        let mut qb = QueryBuilder::new(
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

        let item: Option<ConfigItem> = qb.build_query_as::<ConfigItem>().fetch_optional(&self.pool).await?;

        item.ok_or_else(|| AppError::config_not_found(id))
    }

    pub async fn update_config_item(&self, id: &str, update: UpdateConfigItem, scope: &TenantScope) -> AppResult<ConfigItem> {
        let now = chrono::Utc::now();

        let mut qb = QueryBuilder::new(
            "UPDATE cfg_config_items i \
             JOIN cfg_namespaces n ON i.namespace_id = n.id \
             SET i.value = COALESCE(",
        );
        qb.push_bind(&update.value);
        qb.push(", i.value), i.value_type = COALESCE(");
        qb.push_bind(&update.value_type);
        qb.push(", i.value_type), i.is_secret = COALESCE(");
        qb.push_bind(update.is_secret);
        qb.push(", i.is_secret), i.description = COALESCE(");
        qb.push_bind(&update.description);
        qb.push(", i.description), i.updated_at = ");
        qb.push_bind(now);
        qb.push(" WHERE i.id = ");
        qb.push_bind(id);
        match scope {
            TenantScope::Single(tid) => {
                qb.push(" AND n.tenant_id = ").push_bind(*tid);
            }
            TenantScope::CrossTenant => {}
        }
        qb.build().execute(&self.pool).await?;

        self.get_config_item(id, scope).await
    }

    pub async fn delete_config_item(&self, id: &str, scope: &TenantScope) -> AppResult<()> {
        let mut qb = QueryBuilder::new(
            "DELETE i FROM cfg_config_items i \
             JOIN cfg_namespaces n ON i.namespace_id = n.id WHERE i.id = ",
        );
        qb.push_bind(id);
        match scope {
            TenantScope::Single(tid) => {
                qb.push(" AND n.tenant_id = ").push_bind(*tid);
            }
            TenantScope::CrossTenant => {}
        }
        qb.build().execute(&self.pool).await?;
        Ok(())
    }

    pub async fn query_versions(&self, query: QueryVersions, scope: &TenantScope) -> AppResult<PagedVersions> {
        let page = query.page.unwrap_or(1).max(1);
        let size = query.size.unwrap_or(20).min(100);
        let offset = (page - 1) * size;

        // JOIN cfg_namespaces; hand-written tenant match (and_tenant helper has no alias support).
        let mut qb = QueryBuilder::new(
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
        qb.push(" ORDER BY v.created_at DESC LIMIT ");
        qb.push_bind(size);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        let versions: Vec<ConfigVersion> = qb.build_query_as::<ConfigVersion>().fetch_all(&self.pool).await?;
        let total = versions.len() as i64;

        Ok(PagedVersions { items: versions, total, page, size })
    }
}
