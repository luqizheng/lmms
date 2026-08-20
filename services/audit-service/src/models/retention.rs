use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RetentionPolicy {
    pub hot_days: u32,  // 热数据保留天数（在线）
    pub warm_days: u32, // 温数据保留天数（近线）
    pub cold_days: u32, // 冷数据保留天数（归档）
    pub archive_enabled: bool,
    pub purge_enabled: bool,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            hot_days: 90,
            warm_days: 365,
            cold_days: 365 * 7,
            archive_enabled: true,
            purge_enabled: true,
        }
    }
}

impl RetentionPolicy {
    pub fn new(hot_days: u32, warm_days: u32, cold_days: u32) -> Self {
        Self {
            hot_days,
            warm_days,
            cold_days,
            archive_enabled: true,
            purge_enabled: true,
        }
    }

    pub fn hot_cutoff(&self) -> DateTime<Utc> {
        Utc::now() - Duration::days(self.hot_days as i64)
    }

    pub fn warm_cutoff(&self) -> DateTime<Utc> {
        Utc::now() - Duration::days(self.warm_days as i64)
    }

    pub fn cold_cutoff(&self) -> DateTime<Utc> {
        Utc::now() - Duration::days(self.cold_days as i64)
    }

    pub fn stage(&self, created_at: DateTime<Utc>) -> DataStage {
        let now = Utc::now();
        let age = now.signed_duration_since(created_at);

        if age.num_days() < self.hot_days as i64 {
            DataStage::Hot
        } else if age.num_days() < self.warm_days as i64 {
            DataStage::Warm
        } else if age.num_days() < self.cold_days as i64 {
            DataStage::Cold
        } else {
            DataStage::Expired
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum DataStage {
    Hot,     // 在线，可查询
    Warm,    // 近线，归档中
    Cold,    // 冷归档，查询需恢复
    Expired, // 已过期，待清理
}

impl DataStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Hot => "hot",
            Self::Warm => "warm",
            Self::Cold => "cold",
            Self::Expired => "expired",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RetentionStats {
    pub hot_count: i64,
    pub warm_count: i64,
    pub cold_count: i64,
    pub expired_count: i64,
    pub total_size_bytes: i64,
    pub last_cleanup: Option<DateTime<Utc>>,
}

pub struct RetentionManager;

impl RetentionManager {
    pub fn new() -> Self {
        Self
    }

    pub fn classify_records(
        &self,
        records: &[DateTime<Utc>],
        policy: &RetentionPolicy,
    ) -> RetentionStats {
        let mut stats = RetentionStats {
            hot_count: 0,
            warm_count: 0,
            cold_count: 0,
            expired_count: 0,
            total_size_bytes: 0,
            last_cleanup: None,
        };

        for created_at in records {
            match policy.stage(*created_at) {
                DataStage::Hot => stats.hot_count += 1,
                DataStage::Warm => stats.warm_count += 1,
                DataStage::Cold => stats.cold_count += 1,
                DataStage::Expired => stats.expired_count += 1,
            }
        }

        stats
    }

    pub fn cleanup_sql(&self, policy: &RetentionPolicy, table: &str) -> String {
        format!(
            "DELETE FROM {} WHERE created_at < '{}'",
            table,
            policy.cold_cutoff().format("%Y-%m-%d %H:%M:%S")
        )
    }
}

impl Default for RetentionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retention_policy_stages() {
        let policy = RetentionPolicy::default();

        let now = Utc::now();
        let recent = now - Duration::days(30);
        let old = now - Duration::days(100);
        let very_old = now - Duration::days(400);

        assert_eq!(policy.stage(recent), DataStage::Hot);
        assert_eq!(policy.stage(old), DataStage::Warm);
        assert_eq!(policy.stage(very_old), DataStage::Cold);
    }

    #[test]
    fn test_retention_stats() {
        let manager = RetentionManager::new();
        let policy = RetentionPolicy::default();

        let now = Utc::now();
        let records = vec![
            now - Duration::days(30),  // hot
            now - Duration::days(30),  // hot
            now - Duration::days(100), // warm
            now - Duration::days(400), // cold
        ];

        let stats = manager.classify_records(&records, &policy);

        assert_eq!(stats.hot_count, 2);
        assert_eq!(stats.warm_count, 1);
        assert_eq!(stats.cold_count, 1);
    }
}
