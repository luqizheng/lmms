CREATE TABLE IF NOT EXISTS iam_users (
    id VARCHAR(36) PRIMARY KEY,
    username VARCHAR(50) NOT NULL UNIQUE,
    email VARCHAR(255),
    phone VARCHAR(20),
    password_hash VARCHAR(255) NOT NULL,
    salt VARCHAR(64) NOT NULL,
    nickname VARCHAR(100) NOT NULL,
    avatar VARCHAR(255),
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    phone_verified BOOLEAN NOT NULL DEFAULT FALSE,
    last_login_at DATETIME,
    last_login_ip VARCHAR(50),
    password_expired_at DATETIME,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    deleted_at DATETIME
);

CREATE TABLE IF NOT EXISTS iam_roles (
    id VARCHAR(36) PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    code VARCHAR(50) NOT NULL UNIQUE,
    description TEXT,
    data_scope VARCHAR(20) NOT NULL DEFAULT 'self',
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    deleted_at DATETIME
);

CREATE TABLE IF NOT EXISTS iam_permissions (
    id VARCHAR(36) PRIMARY KEY,
    code VARCHAR(100) NOT NULL UNIQUE,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    category VARCHAR(20) NOT NULL,
    resource VARCHAR(255),
    action VARCHAR(50),
    sort_order INT NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'enabled',
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    deleted_at DATETIME
);

CREATE TABLE IF NOT EXISTS iam_menus (
    id VARCHAR(36) PRIMARY KEY,
    parent_id VARCHAR(36),
    name VARCHAR(100) NOT NULL,
    code VARCHAR(100) NOT NULL UNIQUE,
    path VARCHAR(255) NOT NULL,
    component VARCHAR(255),
    icon VARCHAR(100),
    sort_order INT NOT NULL DEFAULT 0,
    visible BOOLEAN NOT NULL DEFAULT TRUE,
    status VARCHAR(20) NOT NULL DEFAULT 'enabled',
    permission_id VARCHAR(36),
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    deleted_at DATETIME,
    FOREIGN KEY (parent_id) REFERENCES iam_menus(id),
    FOREIGN KEY (permission_id) REFERENCES iam_permissions(id)
);

CREATE TABLE IF NOT EXISTS iam_org_units (
    id VARCHAR(36) PRIMARY KEY,
    parent_id VARCHAR(36),
    name VARCHAR(100) NOT NULL,
    code VARCHAR(50) NOT NULL UNIQUE,
    level INT NOT NULL DEFAULT 1,
    sort_order INT NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    deleted_at DATETIME,
    FOREIGN KEY (parent_id) REFERENCES iam_org_units(id)
);

CREATE TABLE IF NOT EXISTS iam_user_roles (
    user_id VARCHAR(36) NOT NULL,
    role_id VARCHAR(36) NOT NULL,
    created_at DATETIME NOT NULL,
    PRIMARY KEY (user_id, role_id),
    FOREIGN KEY (user_id) REFERENCES iam_users(id),
    FOREIGN KEY (role_id) REFERENCES iam_roles(id)
);

CREATE TABLE IF NOT EXISTS iam_role_permissions (
    role_id VARCHAR(36) NOT NULL,
    permission_id VARCHAR(36) NOT NULL,
    created_at DATETIME NOT NULL,
    PRIMARY KEY (role_id, permission_id),
    FOREIGN KEY (role_id) REFERENCES iam_roles(id),
    FOREIGN KEY (permission_id) REFERENCES iam_permissions(id)
);

CREATE TABLE IF NOT EXISTS iam_user_org_units (
    user_id VARCHAR(36) NOT NULL,
    org_unit_id VARCHAR(36) NOT NULL,
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    created_at DATETIME NOT NULL,
    PRIMARY KEY (user_id, org_unit_id),
    FOREIGN KEY (user_id) REFERENCES iam_users(id),
    FOREIGN KEY (org_unit_id) REFERENCES iam_org_units(id)
);

CREATE TABLE IF NOT EXISTS iam_refresh_tokens (
    id VARCHAR(36) PRIMARY KEY,
    user_id VARCHAR(36) NOT NULL,
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    expires_at DATETIME NOT NULL,
    client_ip VARCHAR(50) NOT NULL,
    user_agent TEXT,
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    revoked_at DATETIME,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    FOREIGN KEY (user_id) REFERENCES iam_users(id)
);

CREATE TABLE IF NOT EXISTS iam_login_logs (
    id VARCHAR(36) PRIMARY KEY,
    user_id VARCHAR(36),
    username VARCHAR(50) NOT NULL,
    ip_address VARCHAR(50) NOT NULL,
    user_agent TEXT,
    location VARCHAR(100),
    login_type VARCHAR(20) NOT NULL,
    success BOOLEAN NOT NULL,
    error_message TEXT,
    created_at DATETIME NOT NULL,
    FOREIGN KEY (user_id) REFERENCES iam_users(id)
);
