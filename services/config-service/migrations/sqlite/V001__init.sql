-- Namespaces: app + env + profile
CREATE TABLE IF NOT EXISTS cfg_namespaces (
    id TEXT PRIMARY KEY,
    app TEXT NOT NULL,
    env TEXT NOT NULL,
    profile TEXT,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_cfg_namespaces_app ON cfg_namespaces(app);
CREATE INDEX IF NOT EXISTS idx_cfg_namespaces_env ON cfg_namespaces(env);

-- Config Items: key-value pairs with type
CREATE TABLE IF NOT EXISTS cfg_config_items (
    id TEXT PRIMARY KEY,
    namespace_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    value_type TEXT NOT NULL DEFAULT 'string',
    is_secret INTEGER NOT NULL DEFAULT 0,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_cfg_config_items_namespace_id ON cfg_config_items(namespace_id);
CREATE INDEX IF NOT EXISTS idx_cfg_config_items_key ON cfg_config_items(key);

-- Config Versions: audit trail for changes
CREATE TABLE IF NOT EXISTS cfg_config_versions (
    id TEXT PRIMARY KEY,
    namespace_id TEXT NOT NULL,
    item_id TEXT NOT NULL,
    key TEXT NOT NULL,
    old_value TEXT NOT NULL,
    new_value TEXT NOT NULL,
    operator TEXT,
    comment TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_cfg_config_versions_namespace_id ON cfg_config_versions(namespace_id);
CREATE INDEX IF NOT EXISTS idx_cfg_config_versions_item_id ON cfg_config_versions(item_id);
