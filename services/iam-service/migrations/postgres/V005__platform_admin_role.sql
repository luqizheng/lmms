-- PG 版：与 mysql V005 语义对齐（TIMESTAMPTZ/BIGINT 有符号/JSONB/独立索引语句）
-- iam-service PostgreSQL V005 — 与 mysql/V005__platform_admin_role.sql 版本号与语义对齐。
--
-- Phase 8.3: platform_admin global role for cross-tenant access.
-- tenant_id=1 (default tenant), same level as tenant_admin but cross_tenant.
-- 幂等：MySQL 侧 ON DUPLICATE KEY 命中 uk_tenant_role_code (tenant_id, code) 或主键 id。
--
-- 转换规则：
--   ON DUPLICATE KEY UPDATE id = id -> ON CONFLICT DO NOTHING
--       （选择依据：该行可能同时命中主键 id 与 uk_tenant_role_code 两个唯一约束，
--        MySQL ON DUPLICATE KEY 对任意唯一键生效，故用不指定目标的裸 DO NOTHING
--        最贴近原语义）
--   INSERT IGNORE ... SELECT -> ON CONFLICT (对应复合主键) DO NOTHING
--       （iam_role_permissions 主键 (role_id, permission_id)，
--        iam_user_roles 主键 (user_id, role_id)，冲突目标确定）
--   NOW() -> CURRENT_TIMESTAMP(3)
--
-- ⚠️ 注释里不得出现 ASCII 分号（执行器按分号切分）。

INSERT INTO iam_roles (id, tenant_id, code, name, description, data_scope, status, created_at, updated_at)
VALUES ('role-platform-admin-seed', 1, 'platform_admin', '平台管理员', '平台管理员（Phase 8.3 跨租户全局 seed）', 'all', 'active', CURRENT_TIMESTAMP(3), CURRENT_TIMESTAMP(3))
ON CONFLICT DO NOTHING;

INSERT INTO iam_role_permissions (role_id, permission_id, created_at)
SELECT 'role-platform-admin-seed', id, CURRENT_TIMESTAMP(3) FROM iam_permissions
WHERE status = 'enabled' AND deleted_at IS NULL
ON CONFLICT (role_id, permission_id) DO NOTHING;

INSERT INTO iam_user_roles (user_id, role_id, created_at)
SELECT u.id, 'role-platform-admin-seed', CURRENT_TIMESTAMP(3) FROM iam_users u
WHERE u.tenant_id = 1 AND u.username = 'admin' AND u.deleted_at IS NULL
ON CONFLICT (user_id, role_id) DO NOTHING;
