use crate::error::AppResult;
use crate::models::{RetentionPolicy, RetentionStats, DataStage};
use common_db::DbPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{info, warn};

pub struct CleanupService {
    pool: DbPool,
    policy: RetentionPolicy,
    running: Arc<RwLock<bool>>,
}

impl CleanupService {
    pub fn new(pool: DbPool, policy: RetentionPolicy) -> Self {
        Self {
            pool,
            policy,
            running: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn get_stats(&self) -> AppResult<RetentionStats> {
        let hot_cutoff = self.policy.hot_cutoff();
        let warm_cutoff = self.policy.warm_cutoff();
        let cold_cutoff = self.policy.cold_cutoff();

        let (hot_count, warm_count, cold_count, expired_count) = match &self.pool {
            #[cfg(feature = "mysql")]
            common_db::DbPool::Mysql(p) => {
                let hot: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM operation_logs WHERE created_at >= ?"
                )
                .bind(hot_cutoff)
                .fetch_one(p)
                .await?;

                let warm: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM operation_logs WHERE created_at < ? AND created_at >= ?"
                )
                .bind(hot_cutoff)
                .bind(warm_cutoff)
                .fetch_one(p)
                .await?;

                let cold: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM operation_logs WHERE created_at < ? AND created_at >= ?"
                )
                .bind(warm_cutoff)
                .bind(cold_cutoff)
                .fetch_one(p)
                .await?;

                let expired: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM operation_logs WHERE created_at < ?"
                )
                .bind(cold_cutoff)
                .fetch_one(p)
                .await?;

                (hot.0, warm.0, cold.0, expired.0)
            }
            #[cfg(feature = "sqlite")]
            common_db::DbPool::Sqlite(p) => {
                let hot: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM operation_logs WHERE created_at >= ?1"
                )
                .bind(hot_cutoff)
                .fetch_one(p)
                .await?;

                let warm: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM operation_logs WHERE created_at < ?1 AND created_at >= ?2"
                )
                .bind(hot_cutoff)
                .bind(warm_cutoff)
                .fetch_one(p)
                .await?;

                let cold: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM operation_logs WHERE created_at < ?1 AND created_at >= ?2"
                )
                .bind(warm_cutoff)
                .bind(cold_cutoff)
                .fetch_one(p)
                .await?;

                let expired: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM operation_logs WHERE created_at < ?1"
                )
                .bind(cold_cutoff)
                .fetch_one(p)
                .await?;

                (hot.0, warm.0, cold.0, expired.0)
            }
        };

        Ok(RetentionStats {
            hot_count,
            warm_count,
            cold_count,
            expired_count,
            total_size_bytes: 0,
            last_cleanup: None,
        })
    }

    pub async fn cleanup_expired(&self) -> AppResult<CleanupResult> {
        let cold_cutoff = self.policy.cold_cutoff();
        let mut deleted_count = 0i64;

        match &self.pool {
            #[cfg(feature = "mysql")]
            common_db::DbPool::Mysql(p) => {
                let result = sqlx::query(
                    "DELETE FROM operation_logs WHERE created_at < ?"
                )
                .bind(cold_cutoff)
                .execute(p)
                .await?;

                deleted_count = result.rows_affected() as i64;
            }
            #[cfg(feature = "sqlite")]
            common_db::DbPool::Sqlite(p) => {
                let result = sqlx::query(
                    "DELETE FROM operation_logs WHERE created_at < ?1"
                )
                .bind(cold_cutoff)
                .execute(p)
                .await?;

                deleted_count = result.rows_affected() as i64;
            }
        }

        info!(
            "Cleanup completed: {} expired records deleted",
            deleted_count
        );

        Ok(CleanupResult {
            deleted_count,
            cutoff_date: cold_cutoff,
        })
    }

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
                
                let cutoff = policy.cold_cutoff();
                let mut deleted = 0i64;

                match &pool {
                    #[cfg(feature = "mysql")]
                    common_db::DbPool::Mysql(p) => {
                        let result = sqlx::query(
                            "DELETE FROM operation_logs WHERE created_at < ?"
                        )
                        .bind(cutoff)
                        .execute(p)
                        .await;

                        if let Ok(r) = result {
                            deleted = r.rows_affected() as i64;
                        }
                    }
                    #[cfg(feature = "sqlite")]
                    common_db::DbPool::Sqlite(p) => {
                        let result = sqlx::query(
                            "DELETE FROM operation_logs WHERE created_at < ?1"
                        )
                        .bind(cutoff)
                        .execute(p)
                        .await;

                        if let Ok(r) = result {
                            deleted = r.rows_affected() as i64;
                        }
                    }
                }

                if deleted > 0 {
                    info!("Scheduled cleanup deleted {} records", deleted);
                }
            }
        });
    }

    pub async fn stop(&self) {
        let mut is_running = self.running.write().await;
        *is_running = false;
    }
}

#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub deleted_count: i64,
    pub cutoff_date: chrono::DateTime<Utc>,
}
