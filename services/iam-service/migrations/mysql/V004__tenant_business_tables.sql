-- Phase 8.2: tenant_id on iam business tables (roles / org_units / login_logs).
-- NOTE: MySQL 8.0 does NOT support IF NOT EXISTS / IF EXISTS on ALTER TABLE
-- clauses (those are MariaDB extensions). Idempotent replay is guaranteed by
-- the sentinel table iam_migration_v004_applied in db.rs::ensure_schema
-- (all-or-nothing, single execution on success).

ALTER TABLE iam_roles
    ADD COLUMN tenant_id BIGINT UNSIGNED NOT NULL DEFAULT 1 AFTER code;

ALTER TABLE iam_roles
    ADD CONSTRAINT fk_iam_roles_tenant FOREIGN KEY (tenant_id) REFERENCES iam_tenants(id);

ALTER TABLE iam_roles
    ADD INDEX idx_iam_roles_tenant (tenant_id);

ALTER TABLE iam_roles
    DROP INDEX code;

ALTER TABLE iam_roles
    ADD UNIQUE KEY uk_tenant_role_code (tenant_id, code);

ALTER TABLE iam_org_units
    ADD COLUMN tenant_id BIGINT UNSIGNED NOT NULL DEFAULT 1 AFTER code;

ALTER TABLE iam_org_units
    ADD CONSTRAINT fk_iam_org_units_tenant FOREIGN KEY (tenant_id) REFERENCES iam_tenants(id);

ALTER TABLE iam_org_units
    ADD INDEX idx_iam_org_units_tenant (tenant_id);

ALTER TABLE iam_org_units
    DROP INDEX code;

ALTER TABLE iam_org_units
    ADD UNIQUE KEY uk_tenant_org_code (tenant_id, code);

ALTER TABLE iam_login_logs
    ADD COLUMN tenant_id BIGINT UNSIGNED NOT NULL DEFAULT 1 AFTER user_id;

ALTER TABLE iam_login_logs
    ADD INDEX idx_iam_login_logs_tenant (tenant_id);

UPDATE iam_roles SET tenant_id = 1 WHERE code = 'tenant_admin';

INSERT IGNORE INTO iam_role_permissions (role_id, permission_id, created_at)
SELECT 'role-tenant-admin-seed', id, NOW() FROM iam_permissions
WHERE status = 'enabled' AND deleted_at IS NULL;
