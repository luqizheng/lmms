-- token-service PostgreSQL V001 — 与 mysql/V001__init.sql **版本号与表结构对齐**。
--
-- 转换规则（设计文档 §3 方言差异归属表）：
--   TIMESTAMP                 -> TIMESTAMPTZ   （§3 #3；Rust 侧是 DateTime<Utc>，
--                                **不是** TIMESTAMP，否则 sqlx 的 type_compatible 不匹配）
--   ON UPDATE CURRENT_TIMESTAMP -> 删除（§3 #18；PG 无列级语法，触发器又与
--                                迁移执行器的 `split('')`/`$$` 不兼容，故改由应用层
--                                在 UPDATE 语句里显式写 updated_at）
--   JSON                      -> JSONB（§3 #5 的落地选择：sqlx 的 `sqlx::types::Json<T>`
--                                以 **JSONB** 类型绑定参数，列若声明成 `JSON` 会在写路径报
--                                42804 "column is of type json but expression is of type jsonb"。
--                                JSONB 与 JSON 语义相同且可索引，读方向 sqlx 两者都兼容）
--   内联 INDEX                -> 独立 CREATE INDEX（§3 #15）
--   ENGINE/CHARSET/COLLATE    -> 删除（PG 无此概念）
--
-- ⚠️ 注释行不得出现 ASCII 分号（迁移执行器先按分号切分、再剥离注释行）。

CREATE TABLE IF NOT EXISTS token_oauth_clients (
  client_id             VARCHAR(64)  PRIMARY KEY,
  client_secret_hash    VARCHAR(255) NOT NULL,
  name                  VARCHAR(128) NOT NULL,
  permissions           JSONB        NOT NULL,
  grant_types           JSONB        NOT NULL,
  access_token_ttl_sec  BIGINT       NOT NULL,
  refresh_token_ttl_sec BIGINT       NOT NULL,
  status                VARCHAR(16)  NOT NULL DEFAULT 'active',
  created_at            TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at            TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS token_auth_codes (
  code        VARCHAR(128) PRIMARY KEY,
  user_id     VARCHAR(64)  NOT NULL,
  expires_at  TIMESTAMPTZ  NOT NULL,
  consumed_at TIMESTAMPTZ  NULL,
  created_at  TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_token_auth_codes_expires
  ON token_auth_codes (expires_at);

CREATE TABLE IF NOT EXISTS token_revoked_jtis (
  jti        VARCHAR(128) PRIMARY KEY,
  expires_at TIMESTAMPTZ  NOT NULL,
  created_at TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_token_revoked_jtis_expires
  ON token_revoked_jtis (expires_at);

CREATE TABLE IF NOT EXISTS token_refresh_markers (
  jti        VARCHAR(128) PRIMARY KEY,
  expires_at TIMESTAMPTZ  NOT NULL,
  created_at TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_token_refresh_markers_expires
  ON token_refresh_markers (expires_at);
