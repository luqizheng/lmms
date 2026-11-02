-- notification-service MySQL V004 — 把 ntf_announcements 的 id / tenant_id 从
-- UNSIGNED 改为**有符号** BIGINT（V001/V002 是 forward-only 已发布迁移，不改它们）。
--
-- 原因（设计文档 §3 #19/#23，D-4 实测）：sqlx-postgres 无任何 unsigned 支持，
-- Rust 侧 id / tenant_id 必须是 i64 才能在 PG 上 bind/try_get（S1 清单 #5）；
-- 而 sqlx-mysql 的 int_compatible 会拒绝 UNSIGNED 标志解码到 i64。因此两侧列都
-- 必须有符号（PG 侧 V001/V002 建表时即无 UNSIGNED，同版本号空操作文件对齐）。
--
-- 完整重写列定义（含 NOT NULL / DEFAULT / AUTO_INCREMENT），否则会改变约束。
-- 幂等性靠 marker 表 ntf_migration_v004_applied（由 ensure_schema 建哨兵表并探测）。

ALTER TABLE ntf_announcements
  MODIFY COLUMN id BIGINT NOT NULL AUTO_INCREMENT,
  MODIFY COLUMN tenant_id BIGINT NOT NULL DEFAULT 1;
