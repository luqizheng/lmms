-- Files: uploaded file metadata
CREATE TABLE IF NOT EXISTS file_files (
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

CREATE INDEX IF NOT EXISTS idx_file_files_tenant ON file_files(tenant_id);
CREATE INDEX IF NOT EXISTS idx_file_files_user ON file_files(user_id);
CREATE INDEX IF NOT EXISTS idx_file_files_expires ON file_files(expires_at);

-- Presign URLs: temporary access URLs
CREATE TABLE IF NOT EXISTS file_presign_urls (
    id TEXT PRIMARY KEY,
    file_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    url TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_by TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_file_presign_urls_file_id ON file_presign_urls(file_id);
CREATE INDEX IF NOT EXISTS idx_file_presign_urls_expires_at ON file_presign_urls(expires_at);