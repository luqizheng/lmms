//! Tenant-scoped query helper.
//!
//! Append `AND tenant_id = ?` to a sqlx `QueryBuilder` so handlers that take
//! `digua_jwt_ctx::TenantScope` can append a tenant filter without hand-binding.
//! `TenantScope::CrossTenant` is a no-op (cross-tenant queries skip the filter).

use digua_jwt_ctx::TenantScope;
use sqlx::{Encode, QueryBuilder, Type};

#[cfg(feature = "mysql")]
pub trait WithTenantScope<'a> {
    fn and_tenant(&mut self, scope: &TenantScope) -> &mut Self;
    fn where_id(
        &mut self,
        id: impl Encode<'a, sqlx::MySql> + Type<sqlx::MySql> + Send + 'a,
    ) -> &mut Self;
}

#[cfg(feature = "mysql")]
impl<'a> WithTenantScope<'a> for QueryBuilder<'a, sqlx::MySql> {
    fn and_tenant(&mut self, scope: &TenantScope) -> &mut Self {
        match scope {
            TenantScope::Single(tid) => {
                self.push(" AND tenant_id = ").push_bind(*tid);
            }
            TenantScope::CrossTenant => {}
        }
        self
    }

    fn where_id(
        &mut self,
        id: impl Encode<'a, sqlx::MySql> + Type<sqlx::MySql> + Send + 'a,
    ) -> &mut Self {
        self.push(" WHERE id = ").push_bind(id);
        self
    }
}
