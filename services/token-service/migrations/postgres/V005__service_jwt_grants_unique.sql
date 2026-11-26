-- token-service PostgreSQL V005 — grant 表复合唯一约束兜底（WP-LMMS-SERVICE-JWT-AUDIENCE-02-R1）。
--
-- 背景：V004 首版（commit 1983c2bb）的 PostgreSQL 脚本漏掉
-- PRIMARY KEY (owner, audience)，已执行过该首版的存量库缺约束。
-- 本迁移对存量库做幂等兜底，与 mysql/V005 同构：
--   1. dup-guard 检查：把 token_service_jwt_grants 全部行 INSERT 进带
--      UNIQUE (owner, audience) 的哨兵表 —— 无重复时每组恰好一行全部通过；
--      存在重复时第二条触发唯一约束、迁移失败停机。错误 detail 会列出
--      重复的 (owner, audience) 值。fail-closed，绝不静默取舍，
--      由运维人工裁决保留行后再重跑本迁移；
--   2. 幂等重建主键：先 DROP CONSTRAINT IF EXISTS 再 ADD。新库
--      （修正版 V004 已含主键）无损通过，旧库补上约束，中途失败重跑亦幂等；
--   3. 哨兵表 token_migration_v005_applied 与既有迁移 marker 探测逻辑一致。
--
-- ⚠️ 注释行不得出现 ASCII 分号（迁移执行器先按分号切分）。

CREATE TABLE IF NOT EXISTS token_v005_grant_dup_guard (
  owner    VARCHAR(64) NOT NULL,
  audience VARCHAR(64) NOT NULL,
  UNIQUE (owner, audience)
);

INSERT INTO token_v005_grant_dup_guard (owner, audience)
SELECT owner, audience FROM token_service_jwt_grants;

ALTER TABLE token_service_jwt_grants DROP CONSTRAINT IF EXISTS token_service_jwt_grants_pkey;

ALTER TABLE token_service_jwt_grants ADD PRIMARY KEY (owner, audience);

DROP TABLE IF EXISTS token_v005_grant_dup_guard;

CREATE TABLE IF NOT EXISTS token_migration_v005_applied (
  applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);