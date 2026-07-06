-- file-service 8.3-fu2: file_files.tenant_id VARCHAR(64) -> BIGINT UNSIGNED
-- 老 schema 用 VARCHAR 存 tenant_id，与平台 BIGINT tenant 模型不一致；
-- 本次 ALTER 直接收紧类型，并 DROP+ADD 重建索引（MODIFY COLUMN 不会自动重建索引类型）。
-- 默认 '1' 回填 NULL/空字符串老数据（dev 期无生产数据，仅防御）。
UPDATE file_files SET tenant_id = 1 WHERE tenant_id IS NULL OR tenant_id = '';
ALTER TABLE file_files
    MODIFY COLUMN tenant_id BIGINT UNSIGNED NOT NULL DEFAULT 1,
    DROP INDEX idx_file_files_tenant,
    ADD INDEX idx_file_files_tenant (tenant_id);