-- Phase 8.4: 系统管理菜单权限码（粗粒度单码）
-- 与 frontend/src/router/modules/system.ts 配合：路由 meta.roles=['platform_admin']
-- 走 framework 既有 filterNoPermissionTree 过滤菜单 + 路由守卫 403。
-- 本权限码 IAM_SYSTEM_READ 作为未来 v-perms 按钮级 / api-gateway 网关层
-- 校验的统一锚点；本任务不直接消费。

INSERT INTO iam_permissions (id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at)
VALUES (
  'perm-system-read',
  'IAM_SYSTEM_READ',
  '系统管理菜单',
  '系统管理（用户/角色/权限/菜单/租户/部门）菜单入口',
  'menu',
  'system',
  'read',
  14,
  'enabled',
  NOW(), NOW()
)
ON DUPLICATE KEY UPDATE
  name = VALUES(name),
  description = VALUES(description),
  sort_order = VALUES(sort_order),
  updated_at = NOW();

-- 把新权限码绑给 V005 已 seed 的 platform_admin 角色。
-- V005 的 INSERT IGNORE ... FROM iam_permissions WHERE status = 'enabled' 是
-- V005 一次性动作，已部署环境不会重跑；V006 必须显式补绑定。
INSERT IGNORE INTO iam_role_permissions (role_id, permission_id, created_at)
SELECT 'role-platform-admin-seed', 'perm-system-read', NOW();
