use crate::error::{AppError, AppErrorExt, AppResult};
use crate::models::config_item::{ConfigItem, CreateConfigItem, PagedConfigItems, QueryConfigItems, UpdateConfigItem};
use crate::models::namespace::{CreateNamespace, Namespace, PagedNamespaces, QueryNamespaces, UpdateNamespace};
use crate::models::version::{ConfigVersion, PagedVersions, QueryVersions};
use common_db::DbPool;
use uuid::Uuid;

pub struct ConfigService {
    pool: DbPool,
}

impl ConfigService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create_namespace(&self, ns: CreateNamespace) -> AppResult<Namespace> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        match &self.pool {
            common_db::DbPool::Mysql(p) => {
                sqlx::query(
                    r#"INSERT INTO cfg_namespaces 
                       (id, app, env, profile, description, created_at, updated_at)
                       VALUES (?, ?, ?, ?, ?, ?, ?)"#
                )
                .bind(&id)
                .bind(&ns.app)
                .bind(&ns.env)
                .bind(&ns.profile)
                .bind(&ns.description)
                .bind(now)
                .bind(now)
                .execute(p)
                .await?;
            }
            common_db::DbPool::Sqlite(p) => {
                sqlx::query(
                    r#"INSERT INTO cfg_namespaces 
                       (id, app, env, profile, description, created_at, updated_at)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#
                )
                .bind(&id)
                .bind(&ns.app)
                .bind(&ns.env)
                .bind(&ns.profile)
                .bind(&ns.description)
                .bind(now)
                .bind(now)
                .execute(p)
                .await?;
            }
        }

        Ok(Namespace {
            id,
            app: ns.app,
            env: ns.env,
            profile: ns.profile,
            description: ns.description,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn query_namespaces(&self, query: QueryNamespaces) -> AppResult<PagedNamespaces> {
        let page = query.page.unwrap_or(1).max(1);
        let size = query.size.unwrap_or(20).min(100);
        let offset = (page - 1) * size;

        let namespaces: Vec<Namespace> = match &self.pool {
            common_db::DbPool::Mysql(p) => {
                sqlx::query_as::<_, Namespace>(
                    "SELECT * FROM cfg_namespaces ORDER BY created_at DESC LIMIT ? OFFSET ?"
                )
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await?
            }
            common_db::DbPool::Sqlite(p) => {
                sqlx::query_as::<_, Namespace>(
                    "SELECT * FROM cfg_namespaces ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
                )
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await?
            }
        };

        let total = namespaces.len() as i64;

        Ok(PagedNamespaces { items: namespaces, total, page, size })
    }

    pub async fn get_namespace(&self, id: &str) -> AppResult<Namespace> {
        let ns: Option<Namespace> = match &self.pool {
            common_db::DbPool::Mysql(p) => {
                sqlx::query_as::<_, Namespace>("SELECT * FROM cfg_namespaces WHERE id = ?")
                    .bind(id)
                    .fetch_optional(p)
                    .await?
            }
            common_db::DbPool::Sqlite(p) => {
                sqlx::query_as::<_, Namespace>("SELECT * FROM cfg_namespaces WHERE id = ?1")
                    .bind(id)
                    .fetch_optional(p)
                    .await?
            }
        };

        ns.ok_or_else(|| AppError::namespace_not_found(id))
    }

    pub async fn update_namespace(&self, id: &str, update: UpdateNamespace) -> AppResult<Namespace> {
        let now = chrono::Utc::now();

        match &self.pool {
            common_db::DbPool::Mysql(p) => {
                sqlx::query(
                    "UPDATE cfg_namespaces SET description = COALESCE(?, description), updated_at = ? WHERE id = ?"
                )
                .bind(&update.description)
                .bind(now)
                .bind(id)
                .execute(p)
                .await?;
            }
            common_db::DbPool::Sqlite(p) => {
                sqlx::query(
                    "UPDATE cfg_namespaces SET description = COALESCE(?1, description), updated_at = ?2 WHERE id = ?3"
                )
                .bind(&update.description)
                .bind(now)
                .bind(id)
                .execute(p)
                .await?;
            }
        }

        self.get_namespace(id).await
    }

    pub async fn delete_namespace(&self, id: &str) -> AppResult<()> {
        match &self.pool {
            common_db::DbPool::Mysql(p) => {
                sqlx::query("DELETE FROM cfg_namespaces WHERE id = ?")
                    .bind(id)
                    .execute(p)
                    .await?;
            }
            common_db::DbPool::Sqlite(p) => {
                sqlx::query("DELETE FROM cfg_namespaces WHERE id = ?1")
                    .bind(id)
                    .execute(p)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn create_config_item(&self, item: CreateConfigItem) -> AppResult<ConfigItem> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        match &self.pool {
            common_db::DbPool::Mysql(p) => {
                sqlx::query(
                    r#"INSERT INTO cfg_config_items 
                       (id, namespace_id, key, value, value_type, is_secret, description, created_at, updated_at)
                       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#
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
                .execute(p)
                .await?;
            }
            common_db::DbPool::Sqlite(p) => {
                sqlx::query(
                    r#"INSERT INTO cfg_config_items 
                       (id, namespace_id, key, value, value_type, is_secret, description, created_at, updated_at)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#
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
                .execute(p)
                .await?;
            }
        }

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
        })
    }

    pub async fn query_config_items(&self, query: QueryConfigItems) -> AppResult<PagedConfigItems> {
        let page = query.page.unwrap_or(1).max(1);
        let size = query.size.unwrap_or(20).min(100);
        let offset = (page - 1) * size;

        let items: Vec<ConfigItem> = match &self.pool {
            common_db::DbPool::Mysql(p) => {
                sqlx::query_as::<_, ConfigItem>(
                    "SELECT * FROM cfg_config_items ORDER BY created_at DESC LIMIT ? OFFSET ?"
                )
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await?
            }
            common_db::DbPool::Sqlite(p) => {
                sqlx::query_as::<_, ConfigItem>(
                    "SELECT * FROM cfg_config_items ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
                )
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await?
            }
        };

        let total = items.len() as i64;

        Ok(PagedConfigItems { items, total, page, size })
    }

    pub async fn get_config_item(&self, id: &str) -> AppResult<ConfigItem> {
        let item: Option<ConfigItem> = match &self.pool {
            common_db::DbPool::Mysql(p) => {
                sqlx::query_as::<_, ConfigItem>("SELECT * FROM cfg_config_items WHERE id = ?")
                    .bind(id)
                    .fetch_optional(p)
                    .await?
            }
            common_db::DbPool::Sqlite(p) => {
                sqlx::query_as::<_, ConfigItem>("SELECT * FROM cfg_config_items WHERE id = ?1")
                    .bind(id)
                    .fetch_optional(p)
                    .await?
            }
        };

        item.ok_or_else(|| AppError::config_not_found(id))
    }

    pub async fn update_config_item(&self, id: &str, update: UpdateConfigItem) -> AppResult<ConfigItem> {
        let now = chrono::Utc::now();

        match &self.pool {
            common_db::DbPool::Mysql(p) => {
                sqlx::query(
                    "UPDATE cfg_config_items SET value = COALESCE(?, value), value_type = COALESCE(?, value_type), is_secret = COALESCE(?, is_secret), description = COALESCE(?, description), updated_at = ? WHERE id = ?"
                )
                .bind(&update.value)
                .bind(&update.value_type)
                .bind(update.is_secret)
                .bind(&update.description)
                .bind(now)
                .bind(id)
                .execute(p)
                .await?;
            }
            common_db::DbPool::Sqlite(p) => {
                sqlx::query(
                    "UPDATE cfg_config_items SET value = COALESCE(?1, value), value_type = COALESCE(?2, value_type), is_secret = COALESCE(?3, is_secret), description = COALESCE(?4, description), updated_at = ?5 WHERE id = ?6"
                )
                .bind(&update.value)
                .bind(&update.value_type)
                .bind(update.is_secret)
                .bind(&update.description)
                .bind(now)
                .bind(id)
                .execute(p)
                .await?;
            }
        }

        self.get_config_item(id).await
    }

    pub async fn delete_config_item(&self, id: &str) -> AppResult<()> {
        match &self.pool {
            common_db::DbPool::Mysql(p) => {
                sqlx::query("DELETE FROM cfg_config_items WHERE id = ?")
                    .bind(id)
                    .execute(p)
                    .await?;
            }
            common_db::DbPool::Sqlite(p) => {
                sqlx::query("DELETE FROM cfg_config_items WHERE id = ?1")
                    .bind(id)
                    .execute(p)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn query_versions(&self, query: QueryVersions) -> AppResult<PagedVersions> {
        let page = query.page.unwrap_or(1).max(1);
        let size = query.size.unwrap_or(20).min(100);
        let offset = (page - 1) * size;

        let versions: Vec<ConfigVersion> = match &self.pool {
            common_db::DbPool::Mysql(p) => {
                sqlx::query_as::<_, ConfigVersion>(
                    "SELECT * FROM cfg_config_versions ORDER BY created_at DESC LIMIT ? OFFSET ?"
                )
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await?
            }
            common_db::DbPool::Sqlite(p) => {
                sqlx::query_as::<_, ConfigVersion>(
                    "SELECT * FROM cfg_config_versions ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
                )
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await?
            }
        };

        let total = versions.len() as i64;

        Ok(PagedVersions { items: versions, total, page, size })
    }
}
