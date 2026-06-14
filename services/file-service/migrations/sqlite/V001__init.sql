-- Files: uploaded file metadata
CREATE TABLE IF NOT EXISTS files (
    id TEXT PRIMARY KEY,
    filename TEXT NOT NULL,
    content_type TEXT,
    size INTEGER NOT NULL,
    hash TEXT,
    storage_path TEXT NOT NULL,
    tenant_id TEXT,
    user_id TEXT,
    is_temporary INTEGER NOT NULL DEFAULT 0,
    expires_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_files_tenant ON files(tenant_id);
CREATE INDEX IF NOT EXISTS idx_files_user ON files(user_id);
CREATE INDEX IF NOT EXISTS idx_files_expires ON files(expires_at);

-- Presign URLs: temporary access URLs
CREATE TABLE IF NOT EXISTS presign_urls (
    id TEXT PRIMARY KEY,
    file_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    url TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_by TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_presign_file ON presign_urls(file_id);
CREATE INDEX IF NOT EXISTS idx_presign_expires ON presign_urls(expires_at);