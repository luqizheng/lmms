-- iam-service MySQL V010 - self-service one-time tokens
-- Runtime executor splits statements on ASCII semicolons, so comments must avoid them.

CREATE TABLE IF NOT EXISTS iam_one_time_tokens (
    id VARCHAR(64) NOT NULL PRIMARY KEY,
    tenant_id BIGINT NOT NULL,
    user_id VARCHAR(64) NOT NULL,
    kind VARCHAR(32) NOT NULL,
    token_hash VARCHAR(64) NOT NULL,
    expires_at DATETIME NOT NULL,
    consumed_at DATETIME NULL,
    created_at DATETIME NOT NULL,
    UNIQUE KEY uq_iam_one_time_token_hash (token_hash),
    KEY idx_iam_one_time_tokens_user (user_id, kind)
);