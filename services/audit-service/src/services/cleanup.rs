use crate::error::AppResult;
use crate::models::{RetentionPolicy, RetentionStats};
use digua_common_db::{db_execute, db_fetch_scalar, DbPool};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::info;

/// 数据保留 / 清理服务。
///
/// 池类型是方言无关的 [`DbPool`]；所有 SQL 走 `digua_common_db` 宏，
/// 因此 `?` → `$N` 的改写与 `u32` 编码问题都由基座层统一处理。
pub struct CleanupService {
    pool: DbPool,
    policy: Arc<RwLock<RetentionPolicy>>,
    running: Arc<RwLock<bool>>,
}

impl CleanupService {
    pub fn new(pool: &DbPool, policy: RetentionPolicy) -> Self {
        Self {
            pool: pool.clone(),
            policy: Arc::new(RwLock::new(policy)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// 按热 / 温 / 冷 / 过期四档统计条数。
    pub async fn get_stats(&self) -> AppResult<RetentionStats> {
        let policy = self.policy.read().await;
        let hot_cutoff = policy.hot_cutoff();
        let warm_cutoff = policy.warm_cutoff();
        let cold_cutoff = policy.cold_cutoff();
        drop(policy);

        let hot: i64 = db_fetch_scalar!(
            &self.pool,
            i64,
            "SELECT COUNT(*) FROM audit_operation_logs WHERE created_at >= ?",
            hot_cutoff
        )?;

        let warm: i64 = db_fetch_scalar!(
            &self.pool,
            i64,
            "SELECT COUNT(*) FROM audit_operation_logs WHERE created_at < ? AND created_at >= ?",
            hot_cutoff,
            warm_cutoff
        )?;

        let cold: i64 = db_fetch_scalar!(
            &self.pool,
            i64,
            "SELECT COUNT(*) FROM audit_operation_logs WHERE created_at < ? AND created_at >= ?",
            warm_cutoff,
            cold_cutoff
        )?;

        let expired: i64 = db_fetch_scalar!(
            &self.pool,
            i64,
            "SELECT COUNT(*) FROM audit_operation_logs WHERE created_at < ?",
            cold_cutoff
        )?;

        Ok(RetentionStats {
            hot_count: hot,
            warm_count: warm,
            cold_count: cold,
            expired_count: expired,
            total_size_bytes: 0,
            last_cleanup: None,
        })
    }

    /// 删除超过冷存期限（cold cutoff）的操作日志。
    pub async fn cleanup_expired(&self) -> AppResult<CleanupResult> {
        let policy = self.policy.read().await;
        let cold_cutoff = policy.cold_cutoff();
        drop(policy);

        let affected = db_execute!(
            &self.pool,
            "DELETE FROM audit_operation_logs WHERE created_at < ?",
            cold_cutoff
        )?;

        let deleted_count = affected as i64;

        info!(
            "Cleanup completed: {} expired records deleted",
            deleted_count
        );

        Ok(CleanupResult {
            deleted_count,
            cutoff_date: cold_cutoff,
        })
    }

    /// 启动后台定时清理任务。
    pub async fn start_background_task(&self, interval_secs: u64) {
        let mut is_running = self.running.write().await;
        *is_running = true;
        drop(is_running);

        let pool = self.pool.clone();
        let policy = self.policy.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(interval_secs));

            loop {
                ticker.tick().await;

                let should_stop = {
                    let r = running.read().await;
                    !*r
                };

                if should_stop {
                    break;
                }

                info!("Running scheduled cleanup task");

                let cutoff = {
                    let p = policy.read().await;
                    p.cold_cutoff()
                };

                let result = db_execute!(
                    &pool,
                    "DELETE FROM audit_operation_logs WHERE created_at < ?",
                    cutoff
                );

                if let Ok(affected) = result {
                    let deleted = affected as i64;
                    if deleted > 0 {
                        info!("Scheduled cleanup deleted {} records", deleted);
                    }
                }
            }
        });
    }

    /// 停止后台定时清理任务。
    pub async fn stop(&self) {
        let mut is_running = self.running.write().await;
        *is_running = false;
    }

    /// 替换保留策略。
    pub async fn update_policy(&self, policy: RetentionPolicy) {
        let mut p = self.policy.write().await;
        *p = policy;
    }

    /// 读取当前保留策略。
    pub async fn get_policy(&self) -> RetentionPolicy {
        self.policy.read().await.clone()
    }
}

#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub deleted_count: i64,
    pub cutoff_date: chrono::DateTime<chrono::Utc>,
}
