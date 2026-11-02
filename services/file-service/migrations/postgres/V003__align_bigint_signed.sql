-- file-service PostgreSQL V003 — **空操作文件**，仅用于与 MySQL 侧版本号对齐。
--
-- PG 的 BIGINT 天然是有符号的，没有 UNSIGNED 概念，
-- 因此 MySQL V003 的 `MODIFY COLUMN tenant_id BIGINT NOT NULL` 在 PG 侧等价于「什么都不做」。
--
-- 为什么仍要存在这个文件：
--   1. 迁移文件数必须与 mysql/ 一一对应（marker 语义与版本号对齐，见 contracts/db-conventions.md）；
--   2. ensure_schema 会按方言选择脚本，PG 侧需要一个可执行的占位；
--   3. marker 表 file_migration_v003_applied 由 ensure_schema 在建表时创建（方言相关 DDL），
--      不依赖任何"忽略错误"逻辑。
--
-- 保持为单条 SELECT，便于执行器按分号切分后得到一条合法语句。

SELECT 1;
