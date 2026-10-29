-- V002: API Key 绑定用户 + 自身过期时间。
-- user_id 可空 = 保持纯机器身份语义，向后兼容既有 API Key。
-- expires_at 可空 = 不过期（与 Figma PAT 的"不设过期"一致）。
--
-- NOTE: MySQL 8.0 does NOT support IF NOT EXISTS on ALTER TABLE ... ADD COLUMN
-- clauses (those are MariaDB extensions). Idempotent replay is guaranteed by
-- the sentinel table token_migration_v002_applied created by this same script
-- (all-or-nothing, single execution on success): db.rs::ensure_schema probes
-- that sentinel, not token_oauth_clients — V001 already created the latter on
-- every deployed database, so it cannot tell whether V002 ever ran.
ALTER TABLE token_oauth_clients
  ADD COLUMN user_id VARCHAR(64) DEFAULT NULL
    COMMENT '绑定的用户 id；client_credentials 流程用它作为 token 的 sub；NULL = 机器身份',
  ADD COLUMN expires_at TIMESTAMP NULL
    COMMENT 'API Key 自身过期时间；NULL = 不过期';

-- 哨兵表：与上面的 ALTER 在同一份脚本里落地，全有或全无。
-- 只有这张表存在，才认为 V002 已应用。
CREATE TABLE IF NOT EXISTS token_migration_v002_applied (
  applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
