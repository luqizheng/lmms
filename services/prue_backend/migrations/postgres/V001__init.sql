-- prue_backend PostgreSQL V001 — 与 mysql/V001__init.sql **版本号与表结构对齐**。
--
-- 转换规则（设计文档 §3 方言差异归属表）：
--   TIMESTAMP                 -> TIMESTAMPTZ   （§3 #3；**不是** TIMESTAMP。
--                                MySQL 侧用 TIMESTAMP，但 Rust 侧是 DateTime<Utc>，
--                                PG 必须 TIMESTAMPTZ 才能匹配 sqlx 的 type_compatible）
--   ON UPDATE CURRENT_TIMESTAMP -> 删除（§3 #18 / Q2-A；PG 无此子句，
--                                由应用层 UPDATE 显式写 updated_at = now()）
--   内联 UNIQUE KEY / INDEX    -> 独立 CREATE INDEX（§3 #15/#16）
--   JSON                       -> 原样（§3 #5；PG 原生支持 json）
--   BOOLEAN                    -> 原样（MySQL 侧已是 BOOLEAN，PG 原生支持）
--
-- ⚠️ 禁止 `$$` / `DO` 块。

CREATE TABLE IF NOT EXISTS prue_menu_configs (
    id VARCHAR(64) PRIMARY KEY,
    version BIGINT NOT NULL,
    name VARCHAR(128) NOT NULL,
    description TEXT,
    routes_json JSON NOT NULL,
    routes_etag CHAR(64) NOT NULL DEFAULT '',
    status VARCHAR(32) NOT NULL DEFAULT 'draft',
    is_active BOOLEAN NOT NULL DEFAULT FALSE,
    created_by VARCHAR(128),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    published_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX uk_prue_menu_configs_version
    ON prue_menu_configs (version);

CREATE INDEX idx_prue_menu_configs_active ON prue_menu_configs (is_active);

CREATE INDEX idx_prue_menu_configs_status ON prue_menu_configs (status);
