-- notification-service PostgreSQL V002 — 与 mysql/V002__ntf_tenant.sql **版本号对齐**。
--
-- MySQL 侧原文（勿改，forward-only）：
--   ALTER TABLE ntf_announcements
--     ADD COLUMN tenant_id BIGINT UNSIGNED NOT NULL DEFAULT 1 AFTER id,
--     ADD INDEX idx_ntf_announcements_tenant (tenant_id)
--
-- 转换规则：
--   BIGINT UNSIGNED -> BIGINT                 （§3 #23，去符号）
--   AFTER id        -> 删除                    （PG 无列序语义）
--   内联 ADD INDEX  -> 独立 CREATE INDEX        （§3 #15）
--   DEFAULT 1 兜底老数据的语义原样保留
--
-- ⚠️ 注释里不得出现 ASCII 分号（迁移执行器按分号切分）。

ALTER TABLE ntf_announcements
  ADD COLUMN tenant_id BIGINT NOT NULL DEFAULT 1;

CREATE INDEX IF NOT EXISTS idx_ntf_announcements_tenant ON ntf_announcements (tenant_id);
