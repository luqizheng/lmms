-- V005: token_service_jwt_grants 复合唯一约束兜底（WP-LMMS-SERVICE-JWT-AUDIENCE-02-R1）。
--
-- 与 postgres/V005 同构。MySQL V004 本就含 PRIMARY KEY (owner, audience)，
-- 正常情况下本迁移是幂等重建（DROP PRIMARY KEY 后原样 ADD），无数据变化：
--   1. dup-guard 检查：把 token_service_jwt_grants 全部行 INSERT 进带
--      UNIQUE KEY 的哨兵表 —— 无重复时每组恰好一行全部通过；存在重复时
--      第二条触发唯一约束（SQLSTATE 23000）、迁移失败停机，错误信息列出
--      重复的 (owner, audience) 值。fail-closed，绝不静默取舍，
--      由运维人工裁决保留行后再重跑本迁移；
--   2. 幂等重建主键：确保复合主键真实存在（防历史 ALTER 半途失败留下的
--      无主键状态）；
--   3. 哨兵表 token_migration_v005_applied 与既有迁移 marker 探测逻辑一致。

CREATE TABLE IF NOT EXISTS token_v005_grant_dup_guard (
  owner    VARCHAR(64) NOT NULL,
  audience VARCHAR(64) NOT NULL,
  UNIQUE KEY uq_token_v005_dup_guard (owner, audience)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

INSERT INTO token_v005_grant_dup_guard (owner, audience)
SELECT owner, audience FROM token_service_jwt_grants;

ALTER TABLE token_service_jwt_grants DROP PRIMARY KEY;

ALTER TABLE token_service_jwt_grants ADD PRIMARY KEY (owner, audience);

DROP TABLE IF EXISTS token_v005_grant_dup_guard;

CREATE TABLE IF NOT EXISTS token_migration_v005_applied (
  applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);