-- token-service PostgreSQL V004 — 与 mysql/V004__service_jwt_grants.sql 对齐。
--
-- WP-LMMS-SERVICE-JWT-AUDIENCE-02-R1：补上 PRIMARY KEY (owner, audience)
--（首版 1983c2bb 漏掉，与 MySQL 侧不一致）。已执行过首版的存量库由
-- V005 幂等兜底（dup-guard 检查 + 重建主键）。
--
-- 方言差异：
--   TIMESTAMP    -> TIMESTAMPTZ
--   ON UPDATE CURRENT_TIMESTAMP 由应用层维护（PG 不写触发器）
--
-- ⚠️ 注释行不得出现 ASCII 分号（迁移执行器先按分号切分）。

CREATE TABLE IF NOT EXISTS token_service_jwt_grants (
  owner          VARCHAR(64)  NOT NULL,
  audience       VARCHAR(64)  NOT NULL,
  status         VARCHAR(16)  NOT NULL DEFAULT 'active',
  system_managed BOOLEAN      NOT NULL DEFAULT FALSE,
  created_at     TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at     TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (owner, audience)
);

CREATE INDEX IF NOT EXISTS idx_token_service_jwt_grants_audience
  ON token_service_jwt_grants (audience);

CREATE TABLE IF NOT EXISTS token_migration_v004_applied (
  applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);