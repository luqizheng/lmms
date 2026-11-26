-- V004: Service JWT audience grants + 管理审计（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01）。
--
-- token_service_jwt_grants: owner → audience 签发授权关系。
-- POST /tokens/service-jwt 在 bootstrap token 校验通过后要求存在一行
-- status='active' 的 grant，否则拒绝签发（默认拒绝，fail-closed）。
--
-- token_service_jwt_grant_audit: 管理变更 append-only 审计（行形对齐
-- audit_events：actor/action/result/request_id/occurred_at），永不携带凭据。

CREATE TABLE IF NOT EXISTS token_service_jwt_grants (
  owner          VARCHAR(64)  NOT NULL,
  audience       VARCHAR(64)  NOT NULL,
  status         VARCHAR(16)  NOT NULL DEFAULT 'active',
  system_managed BOOLEAN      NOT NULL DEFAULT FALSE,
  created_at     TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at     TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (owner, audience),
  INDEX idx_token_service_jwt_grants_status (status)
);

CREATE TABLE IF NOT EXISTS token_service_jwt_grant_audit (
  id            BIGINT       NOT NULL AUTO_INCREMENT PRIMARY KEY,
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
  occurred_at   TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  INDEX idx_token_service_jwt_grant_audit_owner (owner, audience)
);

-- 哨兵表：与 V001..V003 的 ensure_schema marker 探测逻辑保持一致。
CREATE TABLE IF NOT EXISTS token_migration_v004_applied (
  applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
