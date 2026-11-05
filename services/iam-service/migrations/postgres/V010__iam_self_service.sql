-- iam-service PostgreSQL V010 - self-service one-time tokens
-- Runtime executor splits statements on ASCII semicolons, so comments must avoid them.

CREATE TABLE IF NOT EXISTS iam_one_time_tokens (
    id VARCHAR(64) NOT NULL PRIMARY KEY,
    tenant_id BIGINT NOT NULL,
    user_id VARCHAR(64) NOT NULL,
    kind VARCHAR(32) NOT NULL,
    token_hash VARCHAR(64) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_iam_one_time_token_hash ON iam_one_time_tokens (token_hash);

CREATE INDEX IF NOT EXISTS idx_iam_one_time_tokens_user ON iam_one_time_tokens (user_id, kind);