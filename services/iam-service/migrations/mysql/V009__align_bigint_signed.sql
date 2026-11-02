-- iam-service MySQL V009 — 把 iam_tenants.id 与四张表的 tenant_id 从 UNSIGNED
-- 改为**有符号** BIGINT（V001~V008 是 forward-only 已发布迁移，不改它们）。
--
-- 原因（与 notification-service V004 同款，设计文档 §3 #19/#23/D-4 实测）：
-- sqlx-postgres 无任何 unsigned 支持，Rust 侧 tenant_id 必须是 i64 才能在 PG 上
-- bind/try_get（u64→i64 清单）；而 sqlx-mysql 的 int_compatible 会拒绝 UNSIGNED
-- 列解码到 i64。因此两侧列都必须有符号（PG 侧建表时即无 UNSIGNED，
-- 同版本号为空操作文件对齐）。
--
-- FK 列类型变更需先 DROP 再重建（MySQL 8 对被 FK 引用/引用列的直接 MODIFY
-- 报 ERROR 3780）。完整重写列定义（含 NOT NULL / DEFAULT / AUTO_INCREMENT）。
-- 幂等性靠 marker 表 iam_migration_v009_applied（由 ensure_schema 建哨兵表并探测）。

ALTER TABLE iam_users DROP FOREIGN KEY fk_iam_users_tenant;
ALTER TABLE iam_roles DROP FOREIGN KEY fk_iam_roles_tenant;
ALTER TABLE iam_org_units DROP FOREIGN KEY fk_iam_org_units_tenant;

ALTER TABLE iam_tenants
  MODIFY COLUMN id BIGINT NOT NULL AUTO_INCREMENT;
ALTER TABLE iam_users
  MODIFY COLUMN tenant_id BIGINT NOT NULL DEFAULT 1;
ALTER TABLE iam_roles
  MODIFY COLUMN tenant_id BIGINT NOT NULL DEFAULT 1;
ALTER TABLE iam_org_units
  MODIFY COLUMN tenant_id BIGINT NOT NULL DEFAULT 1;
ALTER TABLE iam_login_logs
  MODIFY COLUMN tenant_id BIGINT NOT NULL DEFAULT 1;

ALTER TABLE iam_users
  ADD CONSTRAINT fk_iam_users_tenant FOREIGN KEY (tenant_id) REFERENCES iam_tenants(id);
ALTER TABLE iam_roles
  ADD CONSTRAINT fk_iam_roles_tenant FOREIGN KEY (tenant_id) REFERENCES iam_tenants(id);
ALTER TABLE iam_org_units
  ADD CONSTRAINT fk_iam_org_units_tenant FOREIGN KEY (tenant_id) REFERENCES iam_tenants(id);
