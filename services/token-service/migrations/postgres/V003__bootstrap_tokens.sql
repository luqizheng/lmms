-- token-service PostgreSQL V003 — 与 mysql/V003__bootstrap_tokens.sql 对齐。
--
-- 方言差异：
--   TIMESTAMP    -> TIMESTAMPTZ
--   ON UPDATE CURRENT_TIMESTAMP 由应用层维护（PG AFTER UPDATE 触发器略过）
--   注释不内联（PG 用 COMMENT ON，迁移脚本不写）
--
-- ⚠️ 注释行不得出现 ASCII 分号（迁移执行器先按分号切分）。

CREATE TABLE IF NOT EXISTS token_bootstrap_tokens (
  owner        VARCHAR(64)  PRIMARY KEY,
  token_hash   VARCHAR(128) NOT NULL,
  status       VARCHAR(16)  NOT NULL DEFAULT 'active',
  expires_at   TIMESTAMPTZ  NULL,
  last_used_at TIMESTAMPTZ  NULL,
  created_at   TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at   TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_token_bootstrap_tokens_token
  ON token_bootstrap_tokens (token_hash);

CREATE INDEX IF NOT EXISTS idx_token_bootstrap_tokens_status
  ON token_bootstrap_tokens (status);

CREATE TABLE IF NOT EXISTS token_migration_v003_applied (
  applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
