#![cfg(feature = "mysql")]

use digua_common_db::WithTenantScope;
use digua_jwt_ctx::TenantScope;
use sqlx::{MySql, QueryBuilder};

#[test]
fn appends_tenant_clause_to_query() {
    let mut qb: QueryBuilder<'_, MySql> = QueryBuilder::new(
        "SELECT id FROM iam_users WHERE deleted_at IS NULL",
    );
    qb.and_tenant(&TenantScope::Single(42));
    let sql = qb.sql();
    assert!(sql.contains("AND tenant_id = "), "got: {}", sql);
}

#[test]
fn and_tenant_cross_tenant_is_noop() {
    let mut qb: QueryBuilder<'_, MySql> =
        QueryBuilder::new("SELECT * FROM t WHERE 1=1");
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
