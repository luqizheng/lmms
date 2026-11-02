-- file-service PostgreSQL V001 — 与 mysql/V001__init.sql **版本号与表结构对齐**。
--
-- 转换规则（设计文档 §3 方言差异归属表）：
--   DATETIME        -> TIMESTAMPTZ   （§3 #3；**不是** TIMESTAMP）
--   TINYINT(1)      -> BOOLEAN       （§3 #12）
--   列内联 COMMENT   -> 丢弃          （§3 #14）
--   内联 INDEX       -> 独立 CREATE INDEX （§3 #15）
--   ENGINE/CHARSET  -> 删除
--
-- 注意：本版 `tenant_id` 仍是 VARCHAR(64)，V002 才收紧为 BIGINT —— 与 MySQL 侧一致。
--
-- ⚠️ 禁止 `$$` / `DO` 块。

CREATE TABLE file_files (
    id VARCHAR(36) PRIMARY KEY,
    filename VARCHAR(256) NOT NULL,
    content_type VARCHAR(128),
    size BIGINT NOT NULL,
    hash VARCHAR(64),
    storage_path VARCHAR(512) NOT NULL,
    tenant_id VARCHAR(64),
    user_id VARCHAR(64),
    is_temporary BOOLEAN NOT NULL DEFAULT FALSE,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_file_files_tenant ON file_files (tenant_id);

CREATE INDEX idx_file_files_user ON file_files (user_id);

CREATE INDEX idx_file_files_expires ON file_files (expires_at);

CREATE TABLE file_presign_urls (
    id VARCHAR(36) PRIMARY KEY,
    file_id VARCHAR(36) NOT NULL,
    operation VARCHAR(10) NOT NULL,
    url VARCHAR(512) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_by VARCHAR(64),
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT fk_file_presign_urls_file_id
        FOREIGN KEY (file_id) REFERENCES file_files (id)
);

CREATE INDEX idx_file_presign_urls_file_id ON file_presign_urls (file_id);

CREATE INDEX idx_file_presign_urls_expires_at ON file_presign_urls (expires_at);
