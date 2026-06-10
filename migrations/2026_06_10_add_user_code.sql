-- 2026-06-10 重头做 user-service：member_user 增加 UserCode 字段
-- 业务引用标识（8 位 NanoID），不可用于登录
-- v1 接受短暂锁表（gh-ost/pt-online-schema-change 在 v2 视数据量再考虑）

USE oa3;

ALTER TABLE member_user
  ADD COLUMN UserCode VARCHAR(16) NOT NULL UNIQUE AFTER Id;

-- 已存在数据：回填由 services/user-service/src/bin/backfill_user_code.rs 工具完成
-- （Rust 应用层生成 8 位 NanoID 并 UPDATE，保证唯一性）
