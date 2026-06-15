use crate::error::AppResult;
use crate::models::{RetentionPolicy, RetentionStats};
use sqlx::mysql::MySqlPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::info;

pub struct CleanupService {
    pool: MySqlPool,
    policy: Arc<RwLock<RetentionPolicy>>,
    running: Arc<RwLock<bool>>,
}

impl CleanupService {
    pub fn new(pool: &MySqlPool, policy: RetentionPolicy) -> Self {
        Self {
            pool: pool.clone(),
            policy: Arc::new(RwLock::new(policy)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn get_stats(&self) -> AppResult<RetentionStats> {
        let policy = self.policy.read().await;
        let hot_cutoff = policy.hot_cutoff();
        let warm_cutoff = policy.warm_cutoff();
        let cold_cutoff = policy.cold_cutoff();

        let hot: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM operation_logs WHERE created_at >= ?"
        )
        .bind(hot_cutoff)
        .fetch_one(&self.pool)
        .await?;

        let warm: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM operation_logs WHERE created_at < ? AND created_at >= ?"
        )
        .bind(hot_cutoff)
        .bind(warm_cutoff)
        .fetch_one(&self.pool)
        .await?;

        let cold: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM operation_logs WHERE created_at < ? AND created_at >= ?"
        )
        .bind(warm_cutoff)
        .bind(cold_cutoff)
        .fetch_one(&self.pool)
        .await?;

        let expired: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM operation_logs WHERE created_at < ?"
        )
        .bind(cold_cutoff)
        .fetch_one(&self.pool)
        .await?;

        Ok(RetentionStats {
            hot_count: hot.0,
            warm_count: warm.0,
            cold_count: cold.0,
            expired_count: expired.0,
            total_size_bytes: 0,
            last_cleanup: None,
        })
    }

    pub async fn cleanup_expired(&self) -> AppResult<CleanupResult> {
        let policy = self.policy.read().await;
        let cold_cutoff = policy.cold_cutoff();

        let result = sqlx::query(
            "DELETE FROM operation_logs WHERE created_at < ?"
        )
        .bind(cold_cutoff)
        .execute(&self.pool)
        .await?;

        let deleted_count = result.rows_affected() as i64;

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
                
                let cutoff = {
                    let p = policy.read().await;
                    p.cold_cutoff()
                };

                let result = sqlx::query(
                    "DELETE FROM operation_logs WHERE created_at < ?"
                )
                .bind(cutoff)
                .execute(&pool)
                .await;

                if let Ok(r) = result {
                    let deleted = r.rows_affected() as i64;
                    if deleted > 0 {
                        info!("Scheduled cleanup deleted {} records", deleted);
                    }
                }
            }
        });
    }

    pub async fn stop(&self) {
        let mut is_running = self.running.write().await;
        *is_running = false;
    }

    pub async fn update_policy(&self, policy: RetentionPolicy) {
        let mut p = self.policy.write().await;
        *p = policy;
    }

    pub async fn get_policy(&self) -> RetentionPolicy {
        self.policy.read().await.clone()
    }
}

#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub deleted_count: i64,
    pub cutoff_date: chrono::DateTime<chrono::Utc>,
}
