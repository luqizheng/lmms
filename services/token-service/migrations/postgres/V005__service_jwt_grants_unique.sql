-- token-service PostgreSQL V005 — grant 表复合唯一约束兜底（WP-02-R1/R2）。
--
-- R2 状态感知修复：重复检测（fail-closed，错误含 owner/audience/数量）、
-- 主键三分支判定（已正确存在 → no-op 不做破坏性 DDL；不存在 → 事务性 DDL
-- 建立；定义错误 → 明确失败并报告当前定义）全部在 src/db.rs 的 rust_step
-- （v005_grant_pk_migration）中实现。纯 SQL 无法在本执行器「分号切分 +
-- 逐条自动提交」语义下表达条件 DDL，R1 的 guard 表复制方案已被移除
-- ——guard 表中断残留会使重试永久失败，违反重试安全原则。
--
-- 本文件只做两件事：
--   1. 清理 R1 版 guard 表残留（旧实现中断可能遗留，双方言语法一致），
--      该表是迁移自有的临时对象，不含正式数据；
--   2. 建 marker 哨兵表（与既有迁移 marker 探测逻辑一致）。
--
-- 中断恢复语义：rust_step 任意一步失败或中断 → 哨兵表不存在 → 重试时
-- 重新评估当前状态（重复检测 / 主键判定），无持久残留，安全继续。
--
-- ⚠️ 注释行不得出现 ASCII 分号（迁移执行器先按分号切分）。

DROP TABLE IF EXISTS token_v005_grant_dup_guard;

CREATE TABLE IF NOT EXISTS token_migration_v005_applied (
  applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);