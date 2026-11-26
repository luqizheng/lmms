-- V004: service-jwt owner→audience grants（WP-LMMS-SERVICE-JWT-AUDIENCE-02）。
--
-- 签发面授权表：POST /tokens/service-jwt 在签名前必须确认
-- (canonical owner, audience) 存在 active grant，缺省一律拒绝（fail-closed）。
-- 无通配符、无隐式 owner==audience。
--
-- 内置系统授权由启动 seed 幂等补插（main.rs，listener 就绪前），
-- system_managed = TRUE 的行不属于普通管理面（本阶段不提供 grant CRUD）。

CREATE TABLE IF NOT EXISTS token_service_jwt_grants (
  owner          VARCHAR(64)  NOT NULL,
  audience       VARCHAR(64)  NOT NULL,
  status         VARCHAR(16)  NOT NULL DEFAULT 'active',
  system_managed BOOLEAN      NOT NULL DEFAULT FALSE,
  created_at     TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at     TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (owner, audience),
  INDEX idx_token_service_jwt_grants_audience (audience)
);

-- 哨兵表：与前几版 marker 探测逻辑一致。
CREATE TABLE IF NOT EXISTS token_migration_v004_applied (
  applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);