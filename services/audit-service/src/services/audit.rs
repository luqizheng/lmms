use crate::error::{AppError, AppResult};
use crate::models::login::{CreateLoginLog, LoginLog, PagedLoginLogs, QueryLoginLogs};
use crate::models::operation::{CreateOperationLog, OperationLog, PagedOperationLogs, QueryOperationLogs};
use common_db::DbPool;

pub struct AuditService {
    pool: DbPool,
}

impl AuditService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create_operation_log(&self, log: CreateOperationLog) -> AppResult<i64> {
        match &self.pool {
            #[cfg(feature = "mysql")]
            common_db::DbPool::Mysql(p) => {
                let result = sqlx::query(
                    r#"INSERT INTO operation_logs 
                       (user_id, action, resource, resource_id, before_value, after_value, request_id, ip_address, user_agent)
                       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#
                )
                .bind(&log.user_id)
                .bind(&log.action)
                .bind(&log.resource)
                .bind(&log.resource_id)
                .bind(&log.before_value)
                .bind(&log.after_value)
                .bind(&log.request_id)
                .bind(&log.ip_address)
                .bind(&log.user_agent)
                .execute(p)
                .await?;
                Ok(result.last_insert_id() as i64)
            }
            #[cfg(feature = "sqlite")]
            common_db::DbPool::Sqlite(p) => {
                let result = sqlx::query(
                    r#"INSERT INTO operation_logs 
                       (user_id, action, resource, resource_id, before_value, after_value, request_id, ip_address, user_agent)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#
                )
                .bind(&log.user_id)
                .bind(&log.action)
                .bind(&log.resource)
                .bind(&log.resource_id)
                .bind(&log.before_value)
                .bind(&log.after_value)
                .bind(&log.request_id)
                .bind(&log.ip_address)
                .bind(&log.user_agent)
                .execute(p)
                .await?;
                Ok(result.last_insert_id() as i64)
            }
        }
    }

    pub async fn query_operation_logs(&self, query: QueryOperationLogs) -> AppResult<PagedOperationLogs> {
        let page = query.page.unwrap_or(1).max(1);
        let size = query.size.unwrap_or(20).min(100);
        let offset = (page - 1) * size;

        let logs: Vec<OperationLog> = match &self.pool {
            #[cfg(feature = "mysql")]
            common_db::DbPool::Mysql(p) => {
                sqlx::query_as::<_, OperationLog>(
                    "SELECT * FROM operation_logs ORDER BY created_at DESC LIMIT ? OFFSET ?"
                )
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await?
            }
            #[cfg(feature = "sqlite")]
            common_db::DbPool::Sqlite(p) => {
                sqlx::query_as::<_, OperationLog>(
                    "SELECT * FROM operation_logs ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
                )
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await?
            }
        };

        let total = logs.len() as i64;

        Ok(PagedOperationLogs { items: logs, total, page, size })
    }

    pub async fn create_login_log(&self, log: CreateLoginLog) -> AppResult<i64> {
        match &self.pool {
            #[cfg(feature = "mysql")]
            common_db::DbPool::Mysql(p) => {
                let result = sqlx::query(
                    r#"INSERT INTO login_logs 
                       (user_id, username, success, fail_reason, ip_address, location, user_agent, request_id)
                       VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#
                )
                .bind(&log.user_id)
                .bind(&log.username)
                .bind(log.success)
                .bind(&log.fail_reason)
                .bind(&log.ip_address)
                .bind(&log.location)
                .bind(&log.user_agent)
                .bind(&log.request_id)
                .execute(p)
                .await?;
                Ok(result.last_insert_id() as i64)
            }
            #[cfg(feature = "sqlite")]
            common_db::DbPool::Sqlite(p) => {
                let result = sqlx::query(
                    r#"INSERT INTO login_logs 
                       (user_id, username, success, fail_reason, ip_address, location, user_agent, request_id)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#
                )
                .bind(&log.user_id)
                .bind(&log.username)
                .bind(log.success)
                .bind(&log.fail_reason)
                .bind(&log.ip_address)
                .bind(&log.location)
                .bind(&log.user_agent)
                .bind(&log.request_id)
                .execute(p)
                .await?;
                Ok(result.last_insert_id() as i64)
            }
        }
    }

    pub async fn query_login_logs(&self, query: QueryLoginLogs) -> AppResult<PagedLoginLogs> {
        let page = query.page.unwrap_or(1).max(1);
        let size = query.size.unwrap_or(20).min(100);
        let offset = (page - 1) * size;

        let logs: Vec<LoginLog> = match &self.pool {
            #[cfg(feature = "mysql")]
            common_db::DbPool::Mysql(p) => {
                sqlx::query_as::<_, LoginLog>(
                    "SELECT * FROM login_logs ORDER BY created_at DESC LIMIT ? OFFSET ?"
                )
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await?
            }
            #[cfg(feature = "sqlite")]
            common_db::DbPool::Sqlite(p) => {
                sqlx::query_as::<_, LoginLog>(
                    "SELECT * FROM login_logs ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
                )
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await?
            }
        };

        let total = logs.len() as i64;

        Ok(PagedLoginLogs { items: logs, total, page, size })
    }
}
