use crate::error::AppResult;
use crate::models::{OperationLog, LoginLog, QueryOperationLogs, QueryLoginLogs, PagedOperationLogs, PagedLoginLogs};
use sqlx::mysql::MySqlPool;

pub struct AuditService {
    pool: MySqlPool,
}

impl AuditService {
    pub fn new(pool: &MySqlPool) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn get_user_operation_logs(&self, user_id: &str, limit: i64) -> AppResult<Vec<OperationLog>> {
        let logs = sqlx::query_as::<_, OperationLog>(
            r#"
            SELECT * FROM audit_operation_logs 
            WHERE user_id = ? 
            ORDER BY created_at DESC 
            LIMIT ?
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(logs)
    }

    pub async fn get_user_login_logs(&self, user_id: &str, limit: i64) -> AppResult<Vec<LoginLog>> {
        let logs = sqlx::query_as::<_, LoginLog>(
            r#"
            SELECT * FROM audit_login_logs 
            WHERE user_id = ? 
            ORDER BY created_at DESC 
            LIMIT ?
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(logs)
    }

    pub async fn get_operation_log(&self, id: i64) -> AppResult<Option<OperationLog>> {
        let log = sqlx::query_as::<_, OperationLog>(
            r#"
            SELECT * FROM audit_operation_logs WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(log)
    }

    pub async fn get_login_log(&self, id: i64) -> AppResult<Option<LoginLog>> {
        let log = sqlx::query_as::<_, LoginLog>(
            r#"
            SELECT * FROM audit_login_logs WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(log)
    }

    pub async fn query_operation_logs(&self, query: QueryOperationLogs) -> AppResult<PagedOperationLogs> {
        let page = query.page.unwrap_or(1);
        let size = query.size.unwrap_or(20);
        let offset = ((page - 1) * size) as i64;

        let user_id_ref = query.user_id.as_ref().map(|s| s.as_str());
        let action_ref = query.action.as_ref().map(|s| s.as_str());
        let resource_ref = query.resource.as_ref().map(|s| s.as_str());

        let total = self.count_operation_logs(user_id_ref, action_ref, resource_ref).await?;

        let mut sql = String::from("SELECT * FROM audit_operation_logs WHERE 1=1");

        if user_id_ref.is_some() {
            sql.push_str(" AND user_id = ?");
        }
        if action_ref.is_some() {
            sql.push_str(" AND action = ?");
        }
        if resource_ref.is_some() {
            sql.push_str(" AND resource = ?");
        }
        if query.start_time.is_some() {
            sql.push_str(" AND created_at >= ?");
        }
        if query.end_time.is_some() {
            sql.push_str(" AND created_at <= ?");
        }

        sql.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");

        let mut query_builder = sqlx::query_as::<_, OperationLog>(&sql);

        if let Some(user_id) = user_id_ref {
            query_builder = query_builder.bind(user_id);
        }
        if let Some(action) = action_ref {
            query_builder = query_builder.bind(action);
        }
        if let Some(resource) = resource_ref {
            query_builder = query_builder.bind(resource);
        }
        if let Some(start_time) = query.start_time {
            query_builder = query_builder.bind(start_time);
        }
        if let Some(end_time) = query.end_time {
            query_builder = query_builder.bind(end_time);
        }

        query_builder = query_builder.bind(size as i64).bind(offset);

        let logs = query_builder.fetch_all(&self.pool).await?;

        Ok(PagedOperationLogs {
            items: logs,
            total,
            page,
            size,
        })
    }

    pub async fn query_login_logs(&self, query: QueryLoginLogs) -> AppResult<PagedLoginLogs> {
        let page = query.page.unwrap_or(1);
        let size = query.size.unwrap_or(20);
        let offset = ((page - 1) * size) as i64;

        let username_ref = query.username.as_ref().map(|s| s.as_str());

        let total = self.count_login_logs(username_ref, query.success).await?;

        let mut sql = String::from("SELECT * FROM audit_login_logs WHERE 1=1");

        if username_ref.is_some() {
            sql.push_str(" AND username = ?");
        }
        if query.success.is_some() {
            sql.push_str(" AND success = ?");
        }
        if query.start_time.is_some() {
            sql.push_str(" AND created_at >= ?");
        }
        if query.end_time.is_some() {
            sql.push_str(" AND created_at <= ?");
        }

        sql.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");

        let mut query_builder = sqlx::query_as::<_, LoginLog>(&sql);

        if let Some(username) = username_ref {
            query_builder = query_builder.bind(username);
        }
        if let Some(success) = query.success {
            query_builder = query_builder.bind(success);
        }
        if let Some(start_time) = query.start_time {
            query_builder = query_builder.bind(start_time);
        }
        if let Some(end_time) = query.end_time {
            query_builder = query_builder.bind(end_time);
        }

        query_builder = query_builder.bind(size as i64).bind(offset);

        let logs = query_builder.fetch_all(&self.pool).await?;

        Ok(PagedLoginLogs {
            items: logs,
            total,
            page,
            size,
        })
    }

    async fn count_operation_logs(&self, user_id: Option<&str>, action: Option<&str>, resource: Option<&str>) -> AppResult<i64> {
        let mut sql = String::from("SELECT COUNT(*) FROM audit_operation_logs WHERE 1=1");

        if user_id.is_some() {
            sql.push_str(" AND user_id = ?");
        }
        if action.is_some() {
            sql.push_str(" AND action = ?");
        }
        if resource.is_some() {
            sql.push_str(" AND resource = ?");
        }

        let mut query_builder = sqlx::query_as::<_, (i64,)>(&sql);

        if let Some(user_id) = user_id {
            query_builder = query_builder.bind(user_id);
        }
        if let Some(action) = action {
            query_builder = query_builder.bind(action);
        }
        if let Some(resource) = resource {
            query_builder = query_builder.bind(resource);
        }

        let count = query_builder.fetch_one(&self.pool).await?;
        Ok(count.0)
    }

    async fn count_login_logs(&self, username: Option<&str>, success: Option<bool>) -> AppResult<i64> {
        let mut sql = String::from("SELECT COUNT(*) FROM audit_login_logs WHERE 1=1");

        if username.is_some() {
            sql.push_str(" AND username = ?");
        }
        if success.is_some() {
            sql.push_str(" AND success = ?");
        }

        let mut query_builder = sqlx::query_as::<_, (i64,)>(&sql);

        if let Some(username) = username {
            query_builder = query_builder.bind(username);
        }
        if let Some(success) = success {
            query_builder = query_builder.bind(success);
        }

        let count = query_builder.fetch_one(&self.pool).await?;
        Ok(count.0)
    }

    pub async fn query_logs(
        &self,
        user_id: Option<&str>,
        action: Option<&str>,
        resource: Option<&str>,
        _start_date: Option<chrono::DateTime<chrono::Utc>>,
        _end_date: Option<chrono::DateTime<chrono::Utc>>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<OperationLog>> {
        let mut sql = String::from("SELECT * FROM audit_operation_logs WHERE 1=1");

        if user_id.is_some() {
            sql.push_str(" AND user_id = ?");
        }
        if action.is_some() {
            sql.push_str(" AND action = ?");
        }
        if resource.is_some() {
            sql.push_str(" AND resource = ?");
        }

        sql.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");

        let mut query_builder = sqlx::query_as::<_, OperationLog>(&sql);

        if let Some(uid) = user_id {
            query_builder = query_builder.bind(uid);
        }
        if let Some(act) = action {
            query_builder = query_builder.bind(act);
        }
        if let Some(res) = resource {
            query_builder = query_builder.bind(res);
        }

        query_builder = query_builder.bind(limit).bind(offset);

        let logs = query_builder.fetch_all(&self.pool).await?;

        Ok(logs)
    }
}
