-- PG 版：与 mysql V007 语义对齐（TIMESTAMPTZ/BIGINT 有符号/JSONB/独立索引语句）
-- iam-service PostgreSQL V007 — 与 mysql/V007__system_menu_permission.sql 版本号与语义对齐。
--
-- Phase 8.4: 系统管理菜单权限码（粗粒度单码）
-- 与 frontend/src/router/modules/system.ts 配合：路由 meta.roles=['platform_admin']
-- 走 framework 既有 filterNoPermissionTree 过滤菜单 + 路由守卫 403。
-- 本权限码 IAM_SYSTEM_READ 作为未来 v-perms 按钮级 / api-gateway 网关层
-- 校验的统一锚点，本任务不直接消费。
--
-- 转换规则：
--   ON DUPLICATE KEY UPDATE col = VALUES(col) -> ON CONFLICT (code) DO UPDATE SET col = EXCLUDED.col
--       （选择依据：iam_permissions 的唯一约束是 V001 的列级 UNIQUE(code)，
--        冲突目标确定。MySQL 的 VALUES() 函数改为 PG 的 EXCLUDED 伪表）
--   INSERT IGNORE ... SELECT 常量行 -> ON CONFLICT (role_id, permission_id) DO NOTHING
--       （iam_role_permissions 复合主键即唯一约束。SELECT 无 FROM 在 PG 同样合法）
--   NOW() -> CURRENT_TIMESTAMP(3)
--
-- ⚠️ 注释里不得出现 ASCII 分号（执行器按分号切分）。

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
  CURRENT_TIMESTAMP(3), CURRENT_TIMESTAMP(3)
)
ON CONFLICT (code) DO UPDATE SET
  name = EXCLUDED.name,
  description = EXCLUDED.description,
  sort_order = EXCLUDED.sort_order,
  updated_at = CURRENT_TIMESTAMP(3);

-- 把新权限码绑给 V005 已 seed 的 platform_admin 角色。
-- V005 的幂等绑定（从 iam_permissions 全量 enabled 权限）是 V005 当时的
-- 一次性动作，已部署环境不会重跑，V007 必须显式补绑定（原文如此）。
INSERT INTO iam_role_permissions (role_id, permission_id, created_at)
SELECT 'role-platform-admin-seed', 'perm-system-read', CURRENT_TIMESTAMP(3)
ON CONFLICT (role_id, permission_id) DO NOTHING;
