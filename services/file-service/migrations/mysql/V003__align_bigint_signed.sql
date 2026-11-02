-- file-service MySQL V003 — 把 tenant_id 从 UNSIGNED 改为**有符号** BIGINT。
--
-- 原因（设计文档 §3 #19 / D-4 实测）：sqlx-postgres **没有任何 unsigned 类型支持**，
-- Rust 侧 tenant_id 必须是 i64 才能在 PG 上 bind/try_get；而 sqlx-mysql 的
-- int_compatible 会**拒绝** UNSIGNED 标志解码到 i64。因此两侧列都必须有符号。
--
-- 幂等性：靠 marker 表 file_migration_v003_applied（由 ensure_schema 探测）；
-- 且 MODIFY 到同一类型本身是 no-op，重复执行安全。

ALTER TABLE file_files
    MODIFY COLUMN tenant_id BIGINT NOT NULL DEFAULT 1;
