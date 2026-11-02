-- file-service PostgreSQL V002 — 与 mysql/V002__file_tenant_reconcile.sql **版本号与语义对齐**。
--
-- 目标（与 MySQL 侧一致）：`file_files.tenant_id` 从 VARCHAR(64) 收紧为 BIGINT NOT NULL DEFAULT 1，
-- 并把索引重建到新类型上。
--
-- PG 的差异与解法：
--   1. MySQL 的 `UPDATE ... WHERE tenant_id IS NULL OR tenant_id = ''` 依赖 MySQL 把 '' 与 1 做隐式转换；
--      PG 没有隐式转换，必须**显式**给出字符串字面量，并额外排除非数字内容
--      （`!~ '^[0-9]+$'`），否则下面的 `::bigint` 会因脏数据失败。
--   2. MySQL 用 `MODIFY COLUMN`；PG 用 `ALTER COLUMN ... TYPE ... USING ...`（必须带 USING 才能转类型）。
--   3. MySQL 的「DROP INDEX + ADD INDEX」子句在 PG 里是两条独立 DDL。
--
-- ⚠️ 禁止 `$$` / `DO` 块。

UPDATE file_files
   SET tenant_id = '1'
 WHERE tenant_id IS NULL OR tenant_id = '' OR tenant_id !~ '^[0-9]+$';

ALTER TABLE file_files
    ALTER COLUMN tenant_id TYPE BIGINT USING tenant_id::bigint;

ALTER TABLE file_files ALTER COLUMN tenant_id SET NOT NULL;

ALTER TABLE file_files ALTER COLUMN tenant_id SET DEFAULT 1;

DROP INDEX idx_file_files_tenant;

CREATE INDEX idx_file_files_tenant ON file_files (tenant_id);
