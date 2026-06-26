CREATE TABLE IF NOT EXISTS iam_users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    email TEXT,
    phone TEXT,
    password_hash TEXT NOT NULL,
    salt TEXT NOT NULL,
    nickname TEXT NOT NULL,
    avatar TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    email_verified INTEGER NOT NULL DEFAULT 0,
    phone_verified INTEGER NOT NULL DEFAULT 0,
    last_login_at TEXT,
    last_login_ip TEXT,
    password_expired_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS iam_roles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    description TEXT,
    data_scope TEXT NOT NULL DEFAULT 'self',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS iam_permissions (
    id TEXT PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    category TEXT NOT NULL,
    resource TEXT,
    action TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'enabled',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS iam_menus (
    id TEXT PRIMARY KEY,
    parent_id TEXT,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    path TEXT NOT NULL,
    component TEXT,
    icon TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    visible INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'enabled',
    permission_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    FOREIGN KEY (parent_id) REFERENCES iam_menus(id),
    FOREIGN KEY (permission_id) REFERENCES iam_permissions(id)
);

CREATE TABLE IF NOT EXISTS iam_org_units (
    id TEXT PRIMARY KEY,
    parent_id TEXT,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    level INTEGER NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    FOREIGN KEY (parent_id) REFERENCES iam_org_units(id)
);

CREATE TABLE IF NOT EXISTS iam_user_roles (
    user_id TEXT NOT NULL,
    role_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, role_id),
    FOREIGN KEY (user_id) REFERENCES iam_users(id),
    FOREIGN KEY (role_id) REFERENCES iam_roles(id)
);

CREATE TABLE IF NOT EXISTS iam_role_permissions (
    role_id TEXT NOT NULL,
    permission_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (role_id, permission_id),
    FOREIGN KEY (role_id) REFERENCES iam_roles(id),
    FOREIGN KEY (permission_id) REFERENCES iam_permissions(id)
);

CREATE TABLE IF NOT EXISTS iam_user_org_units (
    user_id TEXT NOT NULL,
    org_unit_id TEXT NOT NULL,
    is_primary INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, org_unit_id),
    FOREIGN KEY (user_id) REFERENCES iam_users(id),
    FOREIGN KEY (org_unit_id) REFERENCES iam_org_units(id)
);

CREATE TABLE IF NOT EXISTS iam_refresh_tokens (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    client_ip TEXT NOT NULL,
    user_agent TEXT,
    revoked INTEGER NOT NULL DEFAULT 0,
    revoked_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES iam_users(id)
);

CREATE TABLE IF NOT EXISTS iam_login_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT,
    username TEXT NOT NULL,
    ip_address TEXT NOT NULL,
    user_agent TEXT,
    location TEXT,
    login_type TEXT NOT NULL,
    success INTEGER NOT NULL,
    error_message TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES iam_users(id)
);
