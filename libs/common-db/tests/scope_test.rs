#![cfg(feature = "mysql")]

use common_db::WithTenantScope;
use jwt_ctx::TenantScope;
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
