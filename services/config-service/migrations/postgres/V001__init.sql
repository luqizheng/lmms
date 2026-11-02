-- config-service PostgreSQL V001 — 与 mysql/V001__init.sql **版本号与表结构对齐**。
--
-- 转换规则（设计文档 §3 方言差异归属表）：
--   DATETIME           -> TIMESTAMPTZ      （§3 #3；**不是** TIMESTAMP，否则 DateTime<Utc> 解码失败）
--   TINYINT(1)         -> BOOLEAN          （§3 #12）
--   列内联 COMMENT      -> 丢弃             （§3 #14）
--   内联 INDEX          -> 独立 CREATE INDEX        （§3 #15）
--   内联 UNIQUE KEY     -> 独立 CREATE UNIQUE INDEX （§3 #16）
--   ENGINE/CHARSET/COLLATE -> 删除（PG 无此概念）
--
-- ⚠️ 迁移执行器先按 ASCII 分号切分、再剥离注释行，因此**注释里也不能出现 ASCII 分号**——
-- 否则切分点落在注释中间，剩余片段不以 -- 开头，会被当成可执行 SQL（首版就踩了这个坑）。
-- 另外禁止 dollar-quote 定界符 / DO 块 / CREATE FUNCTION / CREATE TRIGGER。

CREATE TABLE cfg_namespaces (
    id VARCHAR(36) PRIMARY KEY,
    app VARCHAR(128) NOT NULL,
    env VARCHAR(32) NOT NULL,
    profile VARCHAR(64),
    description VARCHAR(512),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_cfg_namespaces_app ON cfg_namespaces (app);

CREATE INDEX idx_cfg_namespaces_env ON cfg_namespaces (env);

CREATE UNIQUE INDEX uk_cfg_namespaces_app_env_profile
    ON cfg_namespaces (app, env, profile);

CREATE TABLE cfg_config_items (
    id VARCHAR(36) PRIMARY KEY,
    namespace_id VARCHAR(36) NOT NULL,
    "key" VARCHAR(256) NOT NULL,
    value TEXT NOT NULL,
    value_type VARCHAR(32) NOT NULL DEFAULT 'string',
    is_secret BOOLEAN NOT NULL DEFAULT FALSE,
    description VARCHAR(512),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT fk_cfg_config_items_namespace_id
        FOREIGN KEY (namespace_id) REFERENCES cfg_namespaces (id)
);

CREATE INDEX idx_cfg_config_items_namespace_id
    ON cfg_config_items (namespace_id);

CREATE INDEX idx_cfg_config_items_key ON cfg_config_items ("key");

CREATE TABLE cfg_config_versions (
    id VARCHAR(36) PRIMARY KEY,
    namespace_id VARCHAR(36) NOT NULL,
    item_id VARCHAR(36) NOT NULL,
    "key" VARCHAR(256) NOT NULL,
    old_value TEXT NOT NULL,
    new_value TEXT NOT NULL,
    operator VARCHAR(64),
    comment VARCHAR(512),
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_cfg_config_versions_namespace_id
    ON cfg_config_versions (namespace_id);

CREATE INDEX idx_cfg_config_versions_item_id
    ON cfg_config_versions (item_id);

CREATE INDEX idx_cfg_config_versions_created_at
    ON cfg_config_versions (created_at);
