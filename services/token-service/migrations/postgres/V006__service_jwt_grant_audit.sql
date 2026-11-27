-- token-service PostgreSQL V006 — 与 mysql/V006__service_jwt_grant_audit.sql 对齐。
--
-- 方言差异：
--   BIGINT AUTO_INCREMENT PRIMARY KEY -> BIGSERIAL PRIMARY KEY
--   DATETIME -> TIMESTAMPTZ
--
-- 注释行不得出现 ASCII 分号（迁移执行器先按分号切分）

CREATE TABLE IF NOT EXISTS token_service_jwt_grant_audit (
  id            BIGSERIAL    PRIMARY KEY,
  actor_id      VARCHAR(128) NOT NULL,
  actor_type    VARCHAR(16)  NOT NULL,
  owner         VARCHAR(64)  NOT NULL,
  audience      VARCHAR(64)  NOT NULL,
  action        VARCHAR(16)  NOT NULL,
  before_status VARCHAR(16)  NULL,
  after_status  VARCHAR(16)  NULL,
  request_id    VARCHAR(64)  NOT NULL,
  result        VARCHAR(16)  NOT NULL,
  detail        VARCHAR(255) NOT NULL DEFAULT '',
  occurred_at   TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_token_service_jwt_grant_audit_owner
  ON token_service_jwt_grant_audit (owner, audience);

CREATE TABLE IF NOT EXISTS token_migration_v006_applied (
  applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
