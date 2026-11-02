-- PG 版：与 mysql V006 语义对齐（TIMESTAMPTZ/BIGINT 有符号/JSONB/独立索引语句）
-- iam-service PostgreSQL V006 — 与 mysql/V006__org_unit_path.sql 版本号与语义对齐。
--
-- 转换规则：
--   ADD COLUMN ... AFTER status -> 去掉 AFTER（PG 无列序语义）
--   ADD INDEX idx_iam_org_units_path -> CREATE INDEX IF NOT EXISTS
--   UPDATE o JOIN p SET（MySQL 多表 UPDATE）-> PG 的 UPDATE ... FROM
--       （PG 不支持 UPDATE ... JOIN ... SET 语法，语义等价改写）
--   CONCAT(op.path, '/', o.name) -> op.path || '/' || o.name
--       （path / name 均 NOT NULL，两种拼接在非 NULL 下等价）
--   递归 CTE：PG 原生支持 WITH RECURSIVE，结构原样保留，
--       整条语句无内部分号，与按分号切分的执行器兼容
--   ⚠️ 非递归项的 path 必须显式转 text：name 是 varchar(100)，而递归项拼接
--       op.path || '/' || o.name 推断为无界 varchar，PG 要求非递归项先携带
--       最终类型（否则报 42P23 column 4 has type character varying(100)），
--       赋值回 path varchar(500) 时再做长度检查，与 MySQL 语义等价
--
-- ⚠️ 注释里不得出现 ASCII 分号（执行器按分号切分）。

-- Add path column to iam_org_units for fast path-based lookup (e.g. "珠海分公司/销售部")
ALTER TABLE iam_org_units ADD COLUMN path VARCHAR(500) NOT NULL DEFAULT '';

-- Index for path-based lookups
CREATE INDEX IF NOT EXISTS idx_iam_org_units_path ON iam_org_units (path);

-- Backfill existing org unit paths using recursive CTE
WITH RECURSIVE iam_org_paths AS (
    SELECT id, parent_id, name, name::text AS path
    FROM iam_org_units
    WHERE parent_id IS NULL AND deleted_at IS NULL

  UNION ALL

    SELECT o.id, o.parent_id, o.name, op.path || '/' || o.name
    FROM iam_org_units o
    JOIN iam_org_paths op ON o.parent_id = op.id
    WHERE o.deleted_at IS NULL
)
UPDATE iam_org_units o
SET path = p.path
FROM iam_org_paths p
WHERE o.id = p.id;
