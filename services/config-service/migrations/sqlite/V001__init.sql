-- Namespaces: app + env + profile
CREATE TABLE IF NOT EXISTS namespaces (
    id TEXT PRIMARY KEY,
    app TEXT NOT NULL,
    env TEXT NOT NULL,
    profile TEXT,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_ns_app ON namespaces(app);
CREATE INDEX IF NOT EXISTS idx_ns_env ON namespaces(env);

-- Config Items: key-value pairs with type
CREATE TABLE IF NOT EXISTS config_items (
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

CREATE INDEX IF NOT EXISTS idx_item_ns ON config_items(namespace_id);
CREATE INDEX IF NOT EXISTS idx_item_key ON config_items(key);

-- Config Versions: audit trail for changes
CREATE TABLE IF NOT EXISTS config_versions (
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

CREATE INDEX IF NOT EXISTS idx_version_ns ON config_versions(namespace_id);
CREATE INDEX IF NOT EXISTS idx_version_item ON config_versions(item_id);
