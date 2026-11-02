-- notification-service PostgreSQL V003 — 与 mysql/V003__ntf_membership_projection.sql **版本号对齐**。
--
-- 转换规则：
--   DATETIME -> TIMESTAMPTZ（本表无时间列）
--   JSON     -> JSONB      （§3 #5 落地选择：apply_role_assigned 等查询用 jsonb 运算符
--                           （`||` / `-` / `@>`），replace_projection 绑定 serde_json
--                           文本经显式 `?::jsonb` cast 写入。MySQL 侧保持 JSON 不动）
--   内联 INDEX -> 独立 CREATE INDEX（§3 #15）
--   ENGINE / CHARSET / COLLATE -> 删除（§3 #17）
--
-- 索引策略（与 MySQL 侧一致）：
-- - PRIMARY KEY (user_id) — 按 user 唯一，consumer apply 单事务 UPSERT
-- - idx_ntf_user_projection_tenant_dept (tenant_id, dept_id) — resolve_users_by_scopes 的 dept 分支
-- - role_ids 查询走 @> 包含运算全表扫；10K user 量级 < 50ms，超规模再拆表
--
-- ⚠️ 注释里不得出现 ASCII 分号（迁移执行器按分号切分）。

CREATE TABLE IF NOT EXISTS ntf_user_projection (
  user_id        VARCHAR(64) NOT NULL,
  tenant_id      BIGINT      NOT NULL,
  role_ids       JSONB       NOT NULL,
  dept_id        VARCHAR(64) NULL,
  last_event_at  BIGINT      NOT NULL DEFAULT 0,
  PRIMARY KEY (user_id)
);

CREATE INDEX IF NOT EXISTS idx_ntf_user_projection_tenant_dept
  ON ntf_user_projection (tenant_id, dept_id);
