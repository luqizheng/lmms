//! Tenant-scoped query helper.
//!
//! Append `AND tenant_id = ?` to a sqlx `QueryBuilder` so handlers that take
//! `digua_jwt_ctx::TenantScope` can append a tenant filter without hand-binding.
//! `TenantScope::CrossTenant` is a no-op (cross-tenant queries skip the filter).
//!
//! # 为什么是三份「同构重复」
//!
//! `QueryBuilder<DB>` 的 `push_bind` 要求 `Encode<'a, DB> + Type<DB>`，而 `DB`
//! 是**具体类型参数**（`MySql` / `Postgres` / `Sqlite`），Rust 不允许对
//! `QueryBuilder<DB>` 泛型地写出同时满足三者的 `Encode` 约束。
//! 因此这里用关联类型 `Db` 把后端类型参数化，**逻辑只写一份**，
//! 三份 impl 的函数体逐字相同——这是 Rust trait 系统的硬约束导致的唯一重复。

use digua_jwt_ctx::TenantScope;
use sqlx::{Encode, QueryBuilder, Type};

/// 为 `QueryBuilder` 追加租户过滤 / 主键过滤。
///
/// 关联类型 `Db` 指向实际的 sqlx 后端类型，使 `where_id` 的 `Encode` / `Type`
/// 约束能随 impl 一起变化。
pub trait WithTenantScope<'a> {
    /// 本 impl 对应的 sqlx 后端类型。
    type Db: sqlx::Database;

    /// 追加 `AND tenant_id = ?`（`CrossTenant` 时为 no-op）。
    fn and_tenant(&mut self, scope: &TenantScope) -> &mut Self;

    /// 追加 `WHERE id = ?`。
    fn where_id(
        &mut self,
        id: impl Encode<'a, Self::Db> + Type<Self::Db> + Send + 'a,
    ) -> &mut Self;
}

/// MySQL 实现。
#[cfg(feature = "mysql")]
impl<'a> WithTenantScope<'a> for QueryBuilder<'a, sqlx::MySql> {
    type Db = sqlx::MySql;

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
        id: impl Encode<'a, Self::Db> + Type<Self::Db> + Send + 'a,
    ) -> &mut Self {
        self.push(" WHERE id = ").push_bind(id);
        self
    }
}

/// PostgreSQL 实现（与 MySQL 同构，`push_bind` 自动产出 `$1..$N`）。
#[cfg(feature = "postgres")]
impl<'a> WithTenantScope<'a> for QueryBuilder<'a, sqlx::Postgres> {
    type Db = sqlx::Postgres;

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
        id: impl Encode<'a, Self::Db> + Type<Self::Db> + Send + 'a,
    ) -> &mut Self {
        self.push(" WHERE id = ").push_bind(id);
        self
    }
}

/// SQLite 实现（与 MySQL 同构）。
#[cfg(feature = "sqlite")]
impl<'a> WithTenantScope<'a> for QueryBuilder<'a, sqlx::Sqlite> {
    type Db = sqlx::Sqlite;

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
        id: impl Encode<'a, Self::Db> + Type<Self::Db> + Send + 'a,
    ) -> &mut Self {
        self.push(" WHERE id = ").push_bind(id);
        self
    }
}
