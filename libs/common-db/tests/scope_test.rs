//! `WithTenantScope` 的三方言回归测试。
//!
//! 三个 impl 的函数体逐字相同（Rust trait 系统无法对 `QueryBuilder<DB>` 泛型地
//! 写出同时满足 `Encode<'a, DB> + Type<DB>` 的约束），因此这里逐方言各测一遍，
//! 确保「同构复制」不会随时间漂移。

use digua_common_db::WithTenantScope;
use digua_jwt_ctx::TenantScope;

#[cfg(feature = "mysql")]
mod mysql_backend {
    use super::*;
    use sqlx::{MySql, QueryBuilder};

    #[test]
    fn appends_tenant_clause_to_query() {
        let mut qb: QueryBuilder<'_, MySql> =
            QueryBuilder::new("SELECT id FROM iam_users WHERE deleted_at IS NULL");
        qb.and_tenant(&TenantScope::Single(42));
        let sql = qb.sql();
        assert!(sql.contains("AND tenant_id = "), "got: {}", sql);
    }

    #[test]
    fn and_tenant_cross_tenant_is_noop() {
        let mut qb: QueryBuilder<'_, MySql> = QueryBuilder::new("SELECT * FROM t WHERE 1=1");
        qb.and_tenant(&TenantScope::CrossTenant);
        let sql = qb.sql();
        assert!(!sql.contains("tenant_id"), "got: {}", sql);
    }

    #[test]
    fn where_id_appends_where_id_clause() {
        let mut qb: QueryBuilder<'_, MySql> = QueryBuilder::new("SELECT id FROM foo");
        qb.where_id("u_123");
        let sql = qb.sql();
        assert!(sql.contains("WHERE id ="), "got: {}", sql);
    }
}

#[cfg(feature = "postgres")]
mod postgres_backend {
    use super::*;
    use sqlx::{Postgres, QueryBuilder};

    #[test]
    fn appends_tenant_clause_to_query() {
        let mut qb: QueryBuilder<'_, Postgres> =
            QueryBuilder::new("SELECT id FROM iam_users WHERE deleted_at IS NULL");
        qb.and_tenant(&TenantScope::Single(42));
        let sql = qb.sql();
        assert!(sql.contains("AND tenant_id = "), "got: {}", sql);
    }

    #[test]
    fn and_tenant_cross_tenant_is_noop() {
        let mut qb: QueryBuilder<'_, Postgres> = QueryBuilder::new("SELECT * FROM t WHERE 1=1");
        qb.and_tenant(&TenantScope::CrossTenant);
        let sql = qb.sql();
        assert!(!sql.contains("tenant_id"), "got: {}", sql);
    }

    #[test]
    fn where_id_appends_where_id_clause() {
        let mut qb: QueryBuilder<'_, Postgres> = QueryBuilder::new("SELECT id FROM foo");
        qb.where_id("u_123");
        let sql = qb.sql();
        assert!(sql.contains("WHERE id ="), "got: {}", sql);
    }

    /// PG 的 `push_bind` 必须产出**带编号**的占位符（`$1`），否则与
    /// `sql::render` 的改写结果不一致。
    #[test]
    fn push_bind_produces_numbered_placeholder() {
        let mut qb: QueryBuilder<'_, Postgres> =
            QueryBuilder::new("SELECT id FROM foo WHERE id = ");
        qb.push_bind(7i64);
        let sql = qb.sql();
        assert!(sql.contains("$1"), "PG 应产出 $1，实际: {}", sql);
    }
}

#[cfg(feature = "sqlite")]
mod sqlite_backend {
    use super::*;
    use sqlx::{QueryBuilder, Sqlite};

    #[test]
    fn appends_tenant_clause_to_query() {
        let mut qb: QueryBuilder<'_, Sqlite> =
            QueryBuilder::new("SELECT id FROM iam_users WHERE deleted_at IS NULL");
        qb.and_tenant(&TenantScope::Single(42));
        let sql = qb.sql();
        assert!(sql.contains("AND tenant_id = "), "got: {}", sql);
    }

    #[test]
    fn and_tenant_cross_tenant_is_noop() {
        let mut qb: QueryBuilder<'_, Sqlite> = QueryBuilder::new("SELECT * FROM t WHERE 1=1");
        qb.and_tenant(&TenantScope::CrossTenant);
        let sql = qb.sql();
        assert!(!sql.contains("tenant_id"), "got: {}", sql);
    }

    #[test]
    fn where_id_appends_where_id_clause() {
        let mut qb: QueryBuilder<'_, Sqlite> = QueryBuilder::new("SELECT id FROM foo");
        qb.where_id("u_123");
        let sql = qb.sql();
        assert!(sql.contains("WHERE id ="), "got: {}", sql);
    }
}
