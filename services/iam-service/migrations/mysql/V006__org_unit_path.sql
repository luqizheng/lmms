-- Add path column to iam_org_units for fast path-based lookup (e.g. "珠海分公司/销售部")
ALTER TABLE iam_org_units ADD COLUMN path VARCHAR(500) NOT NULL DEFAULT '' AFTER status;

-- Index for path-based lookups
ALTER TABLE iam_org_units ADD INDEX idx_iam_org_units_path (path);

-- Backfill existing org unit paths using recursive CTE
WITH RECURSIVE org_paths AS (
    SELECT id, parent_id, name, name AS path
    FROM iam_org_units
    WHERE parent_id IS NULL AND deleted_at IS NULL

    UNION ALL

    SELECT o.id, o.parent_id, o.name, CONCAT(op.path, '/', o.name)
    FROM iam_org_units o
    JOIN org_paths op ON o.parent_id = op.id
    WHERE o.deleted_at IS NULL
)
UPDATE iam_org_units o
JOIN org_paths p ON o.id = p.id
SET o.path = p.path;
