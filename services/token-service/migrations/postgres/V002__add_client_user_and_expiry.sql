-- token-service PostgreSQL V002 — 与 mysql/V002__add_client_user_and_expiry.sql 对齐。
--
-- 与 MySQL 侧的语义对照：
--   ADD COLUMN user_id    -> 同义（VARCHAR(64) DEFAULT NULL，NULL = 纯机器身份）
--   ADD COLUMN expires_at -> TIMESTAMPTZ NULL（**不是** TIMESTAMP）
--   列内联 COMMENT         -> 丢弃（§3 #14）
--
-- PG 支持 `ADD COLUMN IF NOT EXISTS`，因此本迁移天然幂等。哨兵表仍照旧建立，
-- 使两侧的 marker 语义与版本号完全一致（ensure_schema 探测该哨兵表）。
--
-- ⚠️ 注释行不得出现 ASCII 分号（迁移执行器先按分号切分、再剥离注释行）。

ALTER TABLE token_oauth_clients
  ADD COLUMN IF NOT EXISTS user_id VARCHAR(64) DEFAULT NULL;

ALTER TABLE token_oauth_clients
  ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ NULL;

CREATE TABLE IF NOT EXISTS token_migration_v002_applied (
  applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
