-- V012 (PG): Bootstrap 首管理员认领列（WP-BOOTSTRAP-01-R1 / P1-01 + P1-02）。
--
-- 与 mysql/V012 语义对齐；NULL 不参与 PG 唯一索引（未认领的多行共存）。
--
-- ⚠️ 迁移执行器先按 ASCII 分号切分、再剥离 -- 注释行，注释里不能出现分号；
--   禁止 dollar-quote / DO 块 / CREATE FUNCTION / CREATE TRIGGER。

ALTER TABLE iam_bootstrap_init_requests
    ADD COLUMN admin_user_id VARCHAR(36) NULL
;

CREATE UNIQUE INDEX IF NOT EXISTS uk_iam_bootstrap_admin_user
    ON iam_bootstrap_init_requests (admin_user_id)
;

CREATE TABLE IF NOT EXISTS iam_migration_v012_applied (
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW())
;
