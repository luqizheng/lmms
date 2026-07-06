-- notification-service 8.3-fu2: ntf_announcements 加 tenant_id 列
-- V001__init.sql 漏了，导致 publish_announcement 的 and_tenant 过滤和
-- scheduler.rs 的 `SELECT tenant_id FROM ntf_announcements` 都会 SQL 报错。
-- DEFAULT 1 兜底老数据（dev 期无数据，仅为防御）；点查已切 QueryBuilder+and_tenant。
ALTER TABLE ntf_announcements
  ADD COLUMN tenant_id BIGINT UNSIGNED NOT NULL DEFAULT 1 AFTER id,
  ADD INDEX idx_ntf_announcements_tenant (tenant_id);
