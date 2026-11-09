-- V003: bootstrap tokens 持久化表（Phase 5-B 演进）。
--
-- 取代原先 SERVICE_BOOTSTRAP_TOKENS + SERVICE_BOOTSTRAP_OWNERS 两个 env CSV：
--   1. 每加一个服务只 INSERT 一行，env 不再无限增长；
--   2. owner 与 token 在同一行，天然避免"长度错位"；
--   3. 后续可在此表上实现 rotate / 禁用 / 过期。
--
-- 启动 seed 逻辑：main.rs 从现有 env CSV 读默认值，仅当本表为空时 INSERT，
-- 已存在则跳过——因此 env 可以安全下线（先部署、再删 env）。

CREATE TABLE IF NOT EXISTS token_bootstrap_tokens (
  owner        VARCHAR(64)  PRIMARY KEY,
  token_hash   VARCHAR(128) NOT NULL,
  status       VARCHAR(16)  NOT NULL DEFAULT 'active',
  expires_at   TIMESTAMP    NULL,
  last_used_at TIMESTAMP    NULL,
  created_at   TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at   TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  INDEX idx_token_bootstrap_tokens_token (token_hash),
  INDEX idx_token_bootstrap_tokens_status (status)
);

-- 哨兵表：V002 是 ALTER 类型用了哨兵，这里新建表天然幂等，哨兵只是让
-- ensure_schema 的 marker 探测逻辑与前两版一致。
CREATE TABLE IF NOT EXISTS token_migration_v003_applied (
  applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
