-- config-service MySQL V003 — 把 tenant_id 从 UNSIGNED 改为**有符号** BIGINT。
--
-- 原因（设计文档 §3 #19 / D-4 实测）：sqlx-postgres 无任何 unsigned 支持，Rust 侧
-- tenant_id 必须是 i64 才能在 PG 上 bind/try_get；而 sqlx-mysql 的 int_compatible
-- 会拒绝 UNSIGNED 标志解码到 i64。因此两侧列都必须有符号。
--
-- 完整重写列定义（含 NOT NULL / DEFAULT），否则会改变约束。
-- 幂等性靠 marker 表 cfg_migration_v003_applied（由 ensure_schema 探测）。

ALTER TABLE cfg_namespaces
    MODIFY COLUMN tenant_id BIGINT NOT NULL DEFAULT 1;
