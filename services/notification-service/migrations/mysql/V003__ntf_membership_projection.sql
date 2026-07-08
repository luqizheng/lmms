-- Phase 5B-fu3: notification 本地 user membership 投影表
--
-- 由 notification 启动一次性 GET /iam/internal/membership-bootstrap 全量拉取 + 
-- 持续消费 iam.user.role.{assigned,unassigned} + iam.user.dept.changed 三事件
-- 增量维护，替换 publish_announcement 解析 role/dept scope 的同步 HTTP 路径
-- (spec 2026-07-08-phase5b-fu3-notification-iam-event-sync-design.md §3.3)。
--
-- 索引策略：
-- - PRIMARY KEY (user_id): 按 user 唯一，consumer apply 单事务 UPSERT
-- - idx_tenant_dept (tenant_id, dept_id): resolve_users_by_scopes 的 dept 分支
-- - role_ids 查询走 JSON_CONTAINS 全表扫；10K user 量级 < 50ms。超规模再拆表。
CREATE TABLE IF NOT EXISTS ntf_user_projection (
  user_id        VARCHAR(64) NOT NULL,
  tenant_id      BIGINT      NOT NULL,
  role_ids       JSON        NOT NULL,
  dept_id        VARCHAR(64) NULL,
  last_event_at  BIGINT      NOT NULL DEFAULT 0,
  PRIMARY KEY (user_id),
  KEY idx_ntf_user_projection_tenant_dept (tenant_id, dept_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;