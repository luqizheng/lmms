-- Phase 8.3: platform_admin global role for cross-tenant access.
-- tenant_id=1 (default tenant), same level as tenant_admin but cross_tenant.
-- Idempotent: ON DUPLICATE KEY UPDATE on uk_tenant_role_code (tenant_id, code).

INSERT INTO iam_roles (id, tenant_id, code, name, description, data_scope, status, created_at, updated_at)
VALUES ('role-platform-admin-seed', 1, 'platform_admin', '平台管理员', '平台管理员（Phase 8.3 跨租户全局 seed）', 'all', 'active', NOW(), NOW())
ON DUPLICATE KEY UPDATE id = id;

INSERT IGNORE INTO iam_role_permissions (role_id, permission_id, created_at)
SELECT 'role-platform-admin-seed', id, NOW() FROM iam_permissions
WHERE status = 'enabled' AND deleted_at IS NULL;

INSERT IGNORE INTO iam_user_roles (user_id, role_id, created_at)
SELECT u.id, 'role-platform-admin-seed', NOW() FROM iam_users u
WHERE u.tenant_id = 1 AND u.username = 'admin' AND u.deleted_at IS NULL;
