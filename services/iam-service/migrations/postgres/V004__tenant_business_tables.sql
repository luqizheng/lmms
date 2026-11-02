-- PG 版：与 mysql V004 语义对齐（TIMESTAMPTZ/BIGINT 有符号/JSONB/独立索引语句）
-- iam-service PostgreSQL V004 — 与 mysql/V004__tenant_business_tables.sql 版本号与语义对齐。
--
-- Phase 8.2: tenant_id on iam business tables (roles / org_units / login_logs).
-- 幂等重放由 db.rs::ensure_schema 的哨兵表 iam_migration_v004_applied 保证
-- （单次执行、全有或全无，与 MySQL 侧同款机制，故不依赖语句级 IF NOT EXISTS 兜底，
--  但索引语句仍带 IF NOT EXISTS 与样板一致）。
--
-- 转换规则：
--   ADD COLUMN ... AFTER xxx   -> 去掉 AFTER（PG 无列序语义）
--   ADD CONSTRAINT fk ...      -> PG 同形语法，原样保留
--   ADD INDEX / ADD UNIQUE KEY -> CREATE INDEX / CREATE UNIQUE INDEX IF NOT EXISTS
--   DROP INDEX code（MySQL 对 V001 内联 UNIQUE 的自动索引命名）
--       -> PG 侧对应约束名 iam_roles_code_key / iam_org_units_code_key，
--          用 ALTER TABLE ... DROP CONSTRAINT IF EXISTS 拆除
--   NOW()                      -> CURRENT_TIMESTAMP(3)
--   INSERT IGNORE ... SELECT   -> ON CONFLICT (role_id, permission_id) DO NOTHING
--       （选择依据：iam_role_permissions 的复合主键 (role_id, permission_id)
--        即唯一约束，冲突目标确定，语义与 INSERT IGNORE 等价且幂等）
--
-- ⚠️ 注释里不得出现 ASCII 分号（执行器按分号切分）。

ALTER TABLE iam_roles
    ADD COLUMN tenant_id BIGINT NOT NULL DEFAULT 1;

ALTER TABLE iam_roles
    ADD CONSTRAINT fk_iam_roles_tenant FOREIGN KEY (tenant_id) REFERENCES iam_tenants(id);

CREATE INDEX IF NOT EXISTS idx_iam_roles_tenant ON iam_roles (tenant_id);

-- MySQL: DROP INDEX code
ALTER TABLE iam_roles
    DROP CONSTRAINT IF EXISTS iam_roles_code_key;

-- MySQL: ADD UNIQUE KEY uk_tenant_role_code (tenant_id, code)
CREATE UNIQUE INDEX IF NOT EXISTS uk_tenant_role_code ON iam_roles (tenant_id, code);

ALTER TABLE iam_org_units
    ADD COLUMN tenant_id BIGINT NOT NULL DEFAULT 1;

ALTER TABLE iam_org_units
    ADD CONSTRAINT fk_iam_org_units_tenant FOREIGN KEY (tenant_id) REFERENCES iam_tenants(id);

CREATE INDEX IF NOT EXISTS idx_iam_org_units_tenant ON iam_org_units (tenant_id);

-- MySQL: DROP INDEX code
ALTER TABLE iam_org_units
    DROP CONSTRAINT IF EXISTS iam_org_units_code_key;

-- MySQL: ADD UNIQUE KEY uk_tenant_org_code (tenant_id, code)
CREATE UNIQUE INDEX IF NOT EXISTS uk_tenant_org_code ON iam_org_units (tenant_id, code);

ALTER TABLE iam_login_logs
    ADD COLUMN tenant_id BIGINT NOT NULL DEFAULT 1;

CREATE INDEX IF NOT EXISTS idx_iam_login_logs_tenant ON iam_login_logs (tenant_id);

UPDATE iam_roles SET tenant_id = 1 WHERE code = 'tenant_admin';

-- MySQL: INSERT IGNORE ... SELECT（此刻 iam_permissions 通常为空，插入 0 行，保持幂等）
INSERT INTO iam_role_permissions (role_id, permission_id, created_at)
SELECT 'role-tenant-admin-seed', id, CURRENT_TIMESTAMP(3) FROM iam_permissions
WHERE status = 'enabled' AND deleted_at IS NULL
ON CONFLICT (role_id, permission_id) DO NOTHING;
