-- PG 版：与 mysql V008 语义对齐（TIMESTAMPTZ/BIGINT 有符号/JSONB/独立索引语句）
-- iam-service PostgreSQL V008 — 与 mysql/V008__rebind_tenant_admin_permissions.sql 版本号与语义对齐。
--
-- Phase 8.5 fix: rebind role-tenant-admin-seed to all enabled permissions.
--
-- V004 (Phase 8.2) ran this same idempotent-insert pattern but the table
-- iam_permissions was still empty (V001 is schema-only, no seed data).
-- V007 later introduced the first enabled permission (perm-system-read)
-- but only bound it to role-platform-admin-seed, leaving
-- role-tenant-admin-seed with zero permissions and tripping
-- tenant_register_test::v004_migration_adds_tenant_id_and_rescopes_seed_role
-- (seed tenant_admin must have all enabled permissions, left=0 right=1).
--
-- Replay the V004 binding logic for the tenant_admin seed role.
--
-- 转换规则：
--   INSERT IGNORE ... SELECT -> ON CONFLICT (role_id, permission_id) DO NOTHING
--       （选择依据：iam_role_permissions 的复合主键 (role_id, permission_id)
--        即唯一约束，冲突目标确定，跨重放保持幂等，语义与 INSERT IGNORE 等价）
--   NOW() -> CURRENT_TIMESTAMP(3)
--
-- ⚠️ 注释里不得出现 ASCII 分号（执行器按分号切分）。

INSERT INTO iam_role_permissions (role_id, permission_id, created_at)
SELECT 'role-tenant-admin-seed', id, CURRENT_TIMESTAMP(3) FROM iam_permissions
WHERE status = 'enabled' AND deleted_at IS NULL
ON CONFLICT (role_id, permission_id) DO NOTHING;
