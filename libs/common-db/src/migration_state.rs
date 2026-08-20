//! 数据库迁移状态抽象（Phase 10 — `/internal/migrations/state` 端点支撑）。
//!
//! 7 个 Rust 服务都用"marker table 模式"做迁移（不用 sqlx 内置 `_sqlx_migrations`）：
//!   - 每个 V00X migration 关联一张 marker table（通常是该 V 创建的首张业务表，或一张专用 marker 表）
//!   - 启动时 `ensure_schema` 顺序检查 marker 是否存在，决定是否跑该 V 的 SQL
//!
//! 本模块提供：
//!   - [`MigrationDescriptor`]：(version, name, marker_table) 静态描述
//!   - [`MigrationState`] trait：服务实现以声明自己的迁移清单 + 探测方法
//!   - [`probe_marker_mysql`] / [`probe_marker_sqlite`] helper：查 marker table 是否存在
//!   - [`build_response`]：比对 available vs applied，计算 missing / extra / current_version / in_sync
//!
//! axum handler 不在本模块（避免 digua_common-db 引入 axum 依赖）。
//! 各服务 routes/mod.rs 自行写一个 5-10 行的 thin handler，调用 [`build_response`]。
//!
//! 设计参考：spec `docs/superpowers/specs/2026-07-08-migrations-state-endpoint-design.md`

use serde::Serialize;

/// 单个 migration 的静态描述。
#[derive(Debug, Clone, Copy)]
pub struct MigrationDescriptor {
    /// 版本号，格式 `V001` / `V002` / ...
    pub version: &'static str,
    /// 人类可读名称，例 `init` / `outbox_archive` / `tenants`
    pub name: &'static str,
    /// 用于"探测该 V 是否已应用"的 marker table 名。
    ///
    /// 选择规则：
    ///   - 如果该 V 显式建了 `_applied` marker table（`iam_migration_v00X_applied`）→ 用它
    ///   - 否则用该 V 创建的首张核心业务表（如 `iam_users` / `cfg_namespaces`）
    pub marker_table: &'static str,
}

/// 服务实现的 migration 状态自描述 + 探测 trait。
///
/// 一个服务对应一个 `&'static [MigrationDescriptor]` 数组（编译期常量）。
/// probe_applied 由服务决定如何根据 dialect 探测；helper 见 [`probe_marker_mysql`] / [`probe_marker_sqlite`]。
#[async_trait::async_trait]
pub trait MigrationState: Send + Sync {
    /// 服务名，例 `"iam-service"`。
    fn service_name(&self) -> &'static str;

    /// dialect 字符串，例 `"mysql"` / `"sqlite"` / `"mysql+sqlite"`（用于双 dialect 服务如 prue_backend）。
    fn dialect(&self) -> &'static str;

    /// 该服务的迁移清单（按 version 升序）。
    fn migrations(&self) -> &'static [MigrationDescriptor];
}

/// 探测 MySQL 端单张 marker table 是否存在。
pub async fn probe_marker_mysql(pool: &sqlx::MySqlPool, table: &str) -> Result<bool, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name = ?",
    )
    .bind(table)
    .fetch_one(pool)
    .await?;
    Ok(row.0 > 0)
}

/// 探测 SQLite 端单张 marker table 是否存在。
pub async fn probe_marker_sqlite(
    pool: &sqlx::SqlitePool,
    table: &str,
) -> Result<bool, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table', 'index') AND name = ?",
    )
    .bind(table)
    .fetch_one(pool)
    .await?;
    Ok(row.0 > 0)
}

/// 端点返回 JSON 的序列化结构。
///
/// 字段语义见 spec §2.1。
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct MigrationStateResponse {
    pub service: &'static str,
    pub dialect: &'static str,
    pub available: Vec<MigrationInfo>,
    pub applied: Vec<String>,
    pub current_version: Option<String>,
    pub in_sync: bool,
    pub missing: Vec<String>,
    pub extra: Vec<String>,
}

/// JSON 内 `available` 数组元素的形态。
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct MigrationInfo {
    pub version: &'static str,
    pub name: &'static str,
}

/// 比对 available vs applied，计算响应。
///
/// `applied` 是探测结果（已应用的 version 列表，按探测顺序）。
///
/// 算法：
///   - applied 按 version 字符串升序排序
///   - current_version = sorted(applied).last()，包含 extra 部分
///   - missing = available 中 known 但不在 applied 的
///   - extra = applied 中不在 known 列表的（理想为空）
///   - in_sync = missing 为空 AND extra 为空
pub fn build_response<S: MigrationState + ?Sized>(
    state: &S,
    applied: Vec<String>,
) -> MigrationStateResponse {
    let migrations = state.migrations();

    let available: Vec<MigrationInfo> = migrations
        .iter()
        .map(|m| MigrationInfo {
            version: m.version,
            name: m.name,
        })
        .collect();

    // 已知 versions（升序，源于 migrations 数组的声明顺序）
    let known: std::collections::BTreeSet<&'static str> =
        migrations.iter().map(|m| m.version).collect();

    // applied 中"在 known 里的" → applied_versions；"不在 known 里的" → extra
    let mut applied_known: Vec<&'static str> = Vec::with_capacity(applied.len());
    let mut extra: Vec<String> = Vec::new();
    for v in &applied {
        if known.contains(v.as_str()) {
            applied_known.push(
                migrations
                    .iter()
                    .find(|m| m.version == v.as_str())
                    .unwrap()
                    .version,
            );
        } else {
            extra.push(v.clone());
        }
    }

    // missing = migrations 中 applied 不包含的
    let applied_set: std::collections::BTreeSet<&str> = applied_known.iter().copied().collect();
    let missing: Vec<String> = migrations
        .iter()
        .filter(|m| !applied_set.contains(m.version))
        .map(|m| m.version.to_string())
        .collect();

    // current_version = sorted(applied).last()，包含 extra 部分
    let mut all_applied: Vec<String> = applied.clone();
    all_applied.sort();
    let current_version = all_applied.last().cloned();

    let in_sync = missing.is_empty() && extra.is_empty();

    // applied 字段 = 合并 known + extra，按 version 升序，去重
    let mut all_applied_sorted: Vec<String> = applied_known.iter().map(|s| s.to_string()).collect();
    all_applied_sorted.extend(extra.clone());
    all_applied_sorted.sort();
    all_applied_sorted.dedup();

    MigrationStateResponse {
        service: state.service_name(),
        dialect: state.dialect(),
        available,
        applied: all_applied_sorted,
        current_version,
        in_sync,
        missing,
        extra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestState;
    impl MigrationState for TestState {
        fn service_name(&self) -> &'static str {
            "test-svc"
        }
        fn dialect(&self) -> &'static str {
            "mysql"
        }
        fn migrations(&self) -> &'static [MigrationDescriptor] {
            &[
                MigrationDescriptor {
                    version: "V001",
                    name: "init",
                    marker_table: "t1",
                },
                MigrationDescriptor {
                    version: "V002",
                    name: "two",
                    marker_table: "t2",
                },
                MigrationDescriptor {
                    version: "V003",
                    name: "three",
                    marker_table: "t3",
                },
            ]
        }
    }

    #[test]
    fn all_applied_is_in_sync() {
        let s = TestState;
        let r = build_response(&s, vec!["V001".into(), "V002".into(), "V003".into()]);
        assert!(r.in_sync);
        assert!(r.missing.is_empty());
        assert!(r.extra.is_empty());
        assert_eq!(r.current_version, Some("V003".to_string()));
        assert_eq!(r.applied, vec!["V001", "V002", "V003"]);
    }

    #[test]
    fn missing_when_partial() {
        let s = TestState;
        let r = build_response(&s, vec!["V001".into()]);
        assert!(!r.in_sync);
        assert_eq!(r.missing, vec!["V002".to_string(), "V003".to_string()]);
        assert_eq!(r.current_version, Some("V001".to_string()));
    }

    #[test]
    fn extra_when_db_has_unknown() {
        let s = TestState;
        let r = build_response(&s, vec!["V001".into(), "V999".into()]);
        assert!(!r.in_sync);
        assert_eq!(r.extra, vec!["V999".to_string()]);
        assert_eq!(r.current_version, Some("V999".to_string()));
    }

    #[test]
    fn empty_when_no_migrations_applied() {
        let s = TestState;
        let r = build_response(&s, vec![]);
        assert!(!r.in_sync);
        assert_eq!(
            r.missing,
            vec!["V001".to_string(), "V002".to_string(), "V003".to_string()]
        );
        assert_eq!(r.current_version, None);
    }

    #[test]
    fn applied_sorted_ascending() {
        let s = TestState;
        let r = build_response(&s, vec!["V003".into(), "V001".into(), "V002".into()]);
        assert_eq!(r.applied, vec!["V001", "V002", "V003"]);
        assert_eq!(r.current_version, Some("V003".to_string()));
    }
}
