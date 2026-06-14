-- Files: uploaded file metadata
CREATE TABLE files (
    id VARCHAR(36) PRIMARY KEY,
    filename VARCHAR(256) NOT NULL COMMENT 'File name',
    content_type VARCHAR(128) COMMENT 'Content type',
    size BIGINT NOT NULL COMMENT 'File size in bytes',
    hash VARCHAR(64) COMMENT 'File hash (SHA256)',
    storage_path VARCHAR(512) NOT NULL COMMENT 'Storage path',
    tenant_id VARCHAR(64) COMMENT 'Tenant ID',
    user_id VARCHAR(64) COMMENT 'User ID',
    is_temporary TINYINT(1) NOT NULL DEFAULT 0 COMMENT 'Is temporary file',
    expires_at DATETIME COMMENT 'Expiration time',
    created_at DATETIME NOT NULL DEFAULT UTC_TIMESTAMP(),
    updated_at DATETIME NOT NULL DEFAULT UTC_TIMESTAMP(),
    INDEX idx_files_tenant (tenant_id),
    INDEX idx_files_user (user_id),
    INDEX idx_files_expires (expires_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Presign URLs: temporary access URLs
CREATE TABLE presign_urls (
    id VARCHAR(36) PRIMARY KEY,
    file_id VARCHAR(36) NOT NULL COMMENT 'File ID',
    operation VARCHAR(10) NOT NULL COMMENT 'Operation (get/put)',
    url VARCHAR(512) NOT NULL COMMENT 'Presigned URL',
    expires_at DATETIME NOT NULL COMMENT 'Expiration time',
    created_by VARCHAR(64) COMMENT 'Creator',
    created_at DATETIME NOT NULL DEFAULT UTC_TIMESTAMP(),
    INDEX idx_presign_file (file_id),
    INDEX idx_presign_expires (expires_at),
    CONSTRAINT fk_presign_file FOREIGN KEY (file_id) REFERENCES files(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;