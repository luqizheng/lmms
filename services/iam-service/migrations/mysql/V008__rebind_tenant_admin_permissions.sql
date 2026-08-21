-- Phase 8.5 fix: rebind role-tenant-admin-seed to all enabled permissions.
--
-- V004 (Phase 8.2) ran this same INSERT IGNORE pattern but the table
-- iam_permissions was still empty (V001 is schema-only, no seed data).
-- V007 later introduced the first enabled permission (`perm-system-read`)
-- but only bound it to `role-platform-admin-seed`, leaving
-- `role-tenant-admin-seed` with zero permissions and tripping
-- tenant_register_test::v004_migration_adds_tenant_id_and_rescopes_seed_role
-- (`seed tenant_admin must have all enabled permissions`, left=0 right=1).
--
-- Replay the V004 binding logic for the tenant_admin seed role.
-- INSERT IGNORE keeps it idempotent across re-applies.

INSERT IGNORE INTO iam_role_permissions (role_id, permission_id, created_at)
SELECT 'role-tenant-admin-seed', id, NOW() FROM iam_permissions
WHERE status = 'enabled' AND deleted_at IS NULL;