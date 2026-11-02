-- config-service PostgreSQL V002 — 与 mysql/V002__tenant_cfg.sql **版本号与语义对齐**。
--
-- MySQL 用「一条 ALTER + 多个子句」；PG 的 ALTER TABLE 只能带一个动作，
-- 且索引必须独立 DDL —— 因此这里拆成 4 条独立语句（执行器逐条跑，没问题）。
--
-- 与 MySQL 侧的语义对照：
--   ADD COLUMN tenant_id BIGINT UNSIGNED ... AFTER id
--       -> PG 无 UNSIGNED（§3 #23），用 BIGINT；列顺序在 PG 无语义影响
--          （Namespace 用 #[derive(FromRow)] 按**列名**取，不依赖顺序）
--   ADD INDEX idx_cfg_namespaces_tenant
--       -> CREATE INDEX
--   DROP INDEX uk_cfg_namespaces_app_env_profile
--       -> DROP INDEX
--   ADD UNIQUE KEY uk_cfg_ns_tenant_app_env_profile
--       -> CREATE UNIQUE INDEX
--
-- ⚠️ 禁止 `$$` / `DO` 块。

ALTER TABLE cfg_namespaces ADD COLUMN tenant_id BIGINT NOT NULL DEFAULT 1;

CREATE INDEX idx_cfg_namespaces_tenant ON cfg_namespaces (tenant_id);

DROP INDEX uk_cfg_namespaces_app_env_profile;

CREATE UNIQUE INDEX uk_cfg_ns_tenant_app_env_profile
    ON cfg_namespaces (tenant_id, app, env, profile);
