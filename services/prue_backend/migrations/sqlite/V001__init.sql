CREATE TABLE IF NOT EXISTS menu_configs (
    id TEXT PRIMARY KEY,
    version INTEGER NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT NULL,
    routes_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    is_active INTEGER NOT NULL DEFAULT 0,
    created_by TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    published_at TEXT NULL,
    deleted_at TEXT NULL
);

CREATE INDEX IF NOT EXISTS idx_menu_configs_active ON menu_configs(is_active);
CREATE INDEX IF NOT EXISTS idx_menu_configs_status ON menu_configs(status);
