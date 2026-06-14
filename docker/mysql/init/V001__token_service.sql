-- 1) OAuth 客户端注册表
CREATE TABLE oauth_clients (
  client_id             VARCHAR(64)  PRIMARY KEY,
  client_secret_hash    VARCHAR(255) NOT NULL,
  name                  VARCHAR(128) NOT NULL,
  permissions           JSON         NOT NULL,
  grant_types           JSON         NOT NULL,
  access_token_ttl_sec  BIGINT       NOT NULL,
  refresh_token_ttl_sec BIGINT       NOT NULL,
  status                VARCHAR(16)  NOT NULL DEFAULT 'active',
  created_at            TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at            TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

-- 2) 授权码（替换 SET_EX prefix:code → user_id）
CREATE TABLE auth_codes (
  code        VARCHAR(128) PRIMARY KEY,
  user_id     VARCHAR(64)  NOT NULL,
  expires_at  TIMESTAMP    NOT NULL,
  consumed_at TIMESTAMP    NULL,
  created_at  TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  INDEX idx_auth_codes_expires (expires_at)
);

-- 3) 吊销名单（替换 SET_EX prefix:blacklist:jti）
CREATE TABLE revoked_jtis (
  jti        VARCHAR(128) PRIMARY KEY,
  expires_at TIMESTAMP    NOT NULL,
  created_at TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  INDEX idx_revoked_expires (expires_at)
);

-- 4) refresh 标记（替换 SET_EX prefix:refresh:jti）
CREATE TABLE refresh_markers (
  jti        VARCHAR(128) PRIMARY KEY,
  expires_at TIMESTAMP    NOT NULL,
  created_at TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  INDEX idx_refresh_expires (expires_at)
);
