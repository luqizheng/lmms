-- V012: Bootstrap 首管理员认领列（WP-BOOTSTRAP-01-R1 / P1-01 + P1-02）。
--
-- iam_bootstrap_init_requests.admin_user_id：S02 业务事务内原子认领的
-- 首管理员 user id（非敏感业务元数据，可持久化）。用途：
--   1. S03/S04 无凭据恢复：直接按认领的 user id 绑定角色，不再依赖
--      本次恢复请求提交的用户名/密码（设计 Q7：S02 完成后不重提凭据）；
--   2. 首管理员唯一性的数据库级保护：单行单值 + 唯一索引——同一
--      Bootstrap run 只能绑定一个首管理员，不同用户名并发执行者被
--      request 行锁串行化后命中「已认领」fail-closed 分支。
-- 业务事务必须先 FOR UPDATE 锁定本行并校验 owner/epoch/lease（见
-- executor::lock_and_verify_in_tx），认领 UPDATE 在同一事务内执行。
--
-- ⚠️ 迁移执行器先按 ASCII 分号切分、再剥离 -- 注释行，注释里不能出现分号。

ALTER TABLE iam_bootstrap_init_requests
    ADD COLUMN admin_user_id VARCHAR(36) NULL
;

-- 唯一索引：一个管理员用户只能被一个 Bootstrap run 认领（NULL 不参与
-- MySQL 唯一约束，未认领的多行共存）。
CREATE UNIQUE INDEX uk_iam_bootstrap_admin_user
    ON iam_bootstrap_init_requests (admin_user_id)
;

-- ALTER 型迁移无天然业务表可探测，建哨兵表（与 V004~V010 约定一致）。
CREATE TABLE IF NOT EXISTS iam_migration_v012_applied (
    applied_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6))
;
