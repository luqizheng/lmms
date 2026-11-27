-- V006: service-jwt grant 管理审计（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01）。
--
-- 管理面（routes/service_jwt_grant_{admin,web}.rs）的 append-only 审计：
-- create / enable / disable / delete 的成功与拒绝（denied）均落一行。
-- 行形对齐 audit_events（actor/action/result/request_id/occurred_at），
-- 后续可平移到跨服务审计链路。审计记录永不携带 token/JWT/凭据。
--
-- 注：grant 本体表已在 V004 建立（WP-02），本迁移只加审计表。

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

-- 哨兵表：与既有迁移 marker 探测逻辑一致。
CREATE TABLE IF NOT EXISTS token_migration_v006_applied (
  applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
