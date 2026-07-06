-- config-service 8.3-fu2: cfg_namespaces.tenant_id
-- 老 schema 中无 tenant_id 列，命名空间全局唯一 → 跨租户可读到对方 namespace。
-- 本次 ALTER 添加 tenant_id（默认 1 兜底），并将原 UNIQUE 重建为含 tenant_id 前缀。
-- cfg_config_items / cfg_config_versions 不加冗余列，靠 namespace_id FK 推导。
ALTER TABLE cfg_namespaces
    ADD COLUMN tenant_id BIGINT UNSIGNED NOT NULL DEFAULT 1 AFTER id,
    ADD INDEX idx_cfg_namespaces_tenant (tenant_id),
    DROP INDEX uk_cfg_namespaces_app_env_profile,
    ADD UNIQUE KEY uk_cfg_ns_tenant_app_env_profile (tenant_id, app, env, profile);
