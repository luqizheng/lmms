-- notification-service PostgreSQL V004 — **空操作文件**，仅用于与 MySQL 侧版本号对齐。
--
-- PG 的 BIGINT 天然有符号，没有 UNSIGNED 概念；V001/V002 建表时 id / tenant_id
-- 就已经是普通 BIGINT，因此 MySQL V004 的去 UNSIGNED ALTER 在 PG 侧等价于
-- 「什么都不做」。
--
-- 为什么仍需存在：
--   1. 迁移文件数与版本号必须与 mysql/ 一一对应（marker 语义两端一致）
--   2. ensure_schema 按方言选脚本，PG 侧需要一个可执行占位
--   3. 哨兵表 ntf_migration_v004_applied 由 ensure_schema 建（方言相关 DDL），
--      不依赖任何「忽略错误」逻辑
--
-- 保持单条 SELECT，便于执行器按分号切分后得到一条合法语句。

SELECT 1;
