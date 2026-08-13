---
name: coder-migration-rollback
description: |
  Emergency rollback playbook for forward-only migrations. Use when the user says
  "rollback migration", "迁移回滚", "migrate down", "undo migration", "线上迁移出问题",
  "DB schema 错了", "hotfix migration", "撤销迁移", "V00X__xxx".
---

# coder-migration-rollback Skill

Emergency rollback playbook when a production migration goes wrong.

## 何时用

用户说以下任何一句话时调用本 skill：
- "rollback migration" / "migrate down" / "undo migration"
- "迁移回滚" / "撤销迁移" / "回滚"
- "线上迁移出问题" / "DB schema 错了" / "表结构错了"
- "hotfix migration" / "紧急修复迁移"
- 提及具体 V{N} 文件名（如 V006__org_unit_path）

## 前置

- 运行环境：WSL / macOS / Linux
- 工作目录：项目根目录
- 确认当前数据库状态：`node scripts/check-db-naming.js` 无报错
- MySQL 已启动（`pnpm run dev:infra`）
- 已阅读 `AGENTS.md` § 数据库迁移

## 核心原则

### Forward-Only 铁律

**禁止编辑已发布的 V{N}__.sql 文件。** 理由：生产环境可能已应用该迁移，编辑造成代码与 prod schema 漂移，且可能与下游消费仓的镜像不同步。

### 补偿迁移（Compensating Migration）

回滚路径永远是写一个新的 V{N+1}__*.sql，向补偿错误变更，而不是修改原始迁移。

```
V006 (bad) → V007 (revert V006) → V008 (fix)  ✓正确路径
V006 (bad) → 编辑V006                     ✗禁止
```

### prue_backend 特殊规则

prue_backend 必须同时维护 `migrations/mysql/` 和 `migrations/sqlite/` 两套迁移。回滚时**必须**同时写两套 V{N+1}，否则本地 SQLite dev 与线上 MySQL 不一致。

## 决策树

```
线上迁移出错
  │
  ├─ 新加列（ADD COLUMN）
  │    └─ 写 V{N+1}__drop_unused_column.sql:
  │         ALTER TABLE <prefix>_<table> DROP COLUMN IF EXISTS <col_name>;
  │
  ├─ 新加表（CREATE TABLE）
  │    └─ 写 V{N+1}__drop_table.sql:
  │         DROP TABLE IF EXISTS <prefix>_<table>;
  │
  ├─ 删列（DROP COLUMN）
  │    └─ ⚠️ 不可逆，数据已丢失
  │       1. mysqldump 备份
  │       2. 从备份 restore 到临时库
  │       3. 业务确认数据完整性
  │       4. V{N+1}__recover_column.sql 重建列 + 恢复数据
  │
  ├─ 改类型（ALTER COLUMN TYPE）
  │    └─ 写 V{N+1}__revert_type.sql（反向 ALTER）
  │         BIGINT → INT:    ALTER TABLE <t> MODIFY <c> INT NOT NULL;
  │         VARCHAR(500)→VARCHAR(255): ALTER TABLE <t> MODIFY <c> VARCHAR(255);
  │
  ├─ 索引变更（CREATE/DROP INDEX）
  │    └─ CREATE INDEX 错误 → V{N+1}__drop_index.sql:
  │         DROP INDEX <idx_name> ON <prefix>_<table>;
  │       DROP INDEX 误删 → V{N+1}__recreate_index.sql:
  │         CREATE INDEX <idx_name> ON <prefix>_<table>(<col>);
  │
  └─ 数据迁移（大批量 UPDATE/INSERT/DELETE）
       └─ 必须先 mysqldump 备份，回滚靠 restore + 新 V{N+1}
```

## SQL 模板

### 丢弃新增列

```sql
-- V{N+1}__drop_unused_column.sql
-- 回滚风险评估：立即删除列及数据，无UNDO；务必先 SELECT 确认数据可弃
ALTER TABLE <prefix>_<table> DROP COLUMN IF EXISTS <col_name>;
```

### 丢弃新增表

```sql
-- V{N+1}__drop_table.sql
-- 回滚风险评估：立即删除整表及数据，无UNDO；务必先 mysqldump 备份
DROP TABLE IF EXISTS <prefix>_<table>;
```

### 重建误删列（不可逆场景的最后一搏）

```sql
-- V{N+1}__recover_column.sql
-- 回滚风险评估：依赖备份完整性；若备份缺失则数据永久丢失
ALTER TABLE <prefix>_<table> ADD COLUMN <col_name> <type> <nullable>;
-- 从备份恢复数据（若备份完整）
-- UPDATE <prefix>_<table> t JOIN backup_table b ON t.id = b.id SET t.<col_name> = b.<col_name>;
```

### 反向修改列类型

```sql
-- V{N+1}__revert_type.sql
-- 回滚风险评估：数据截断风险；若新类型更窄可能导致数据丢失
-- BIGINT → INT（MySQL）
ALTER TABLE <prefix>_<table> MODIFY <col_name> INT NOT NULL;

-- VARCHAR(500) → VARCHAR(255)
ALTER TABLE <prefix>_<table> MODIFY <col_name> VARCHAR(255);

-- 若为 prue_backend 的 SQLite 版本
-- SQLite 不支持直接 MODIFY，需重建表：
ALTER TABLE <prefix>_<table> RENAME TO <prefix>_<table>_old;
CREATE TABLE <prefix>_<table> (
    id TEXT PRIMARY KEY,
    <col_name> <original_type>,  -- 写原始类型
    ...其他列...
);
INSERT INTO <prefix>_<table> SELECT * FROM <prefix>_<table>_old;
DROP TABLE <prefix>_<table>_old;
```

### 丢弃错误索引

```sql
-- V{N+1}__drop_index.sql
-- 回滚风险评估：删除索引不影响数据，只影响查询性能
DROP INDEX <idx_name> ON <prefix>_<table>;
```

### 重建误删索引

```sql
-- V{N+1}__recreate_index.sql
-- 回滚风险评估：索引重建需写锁，对大表可能阻塞写入
CREATE INDEX <idx_name> ON <prefix>_<table>(<col_name>);
```

### 标记回滚状态（数据迁移场景）

```sql
-- V{N+1}__mark_rollback.sql
-- 用于业务层已回滚但需在 DB 记录状态
INSERT INTO <prefix>_<table>_rollback_log (id, action, rolled_back_at, original_data)
SELECT id, 'BULK_ROLLBACK', NOW(), json_data FROM <prefix>_<table>;
```

## 数据修复 vs Schema 修复

| 场景 | 是否需要 mysqldump 备份 |
|------|------------------------|
| DROP COLUMN | 必须（数据已丢） |
| DROP TABLE | 必须（整表已丢） |
| ALTER COLUMN TYPE | 建议（可能丢数据） |
| CREATE/DROP INDEX | 不需要（只影响性能） |
| 批量数据迁移错误 | 必须（可从备份恢复） |

**备份命令（MySQL）：**

```bash
# 全库备份（生产高危操作，建议在维护窗口做）
mysqldump -h localhost -u root -p --single-transaction --routines --triggers superpower > backup_$(date +%Y%m%d_%H%M%S).sql

# 单表备份
mysqldump -h localhost -u root -p --single-transaction <prefix>_<table> > backup_table_$(date +%Y%m%d_%H%M%S).sql

# 从备份恢复（先在临时库验证）
mysql -h localhost -u root -p superpower < backup_xxx.sql
```

## 紧急 Hotfix 工作流

### Step 1：灰度关停写入

```
# 方式A：关停服务实例（推荐）
kubectl scale deployment <svc> --replicas=0

# 方式B： Consul 摘除节点（不停写）
consul-maintenance -enable -service=<svc>
```

### Step 2：备份（必须先做）

```bash
mysqldump -h <host> -u root -p --single-transaction superpower > /tmp/backup_$(date +%Y%m%d_%H%M%S).sql
```

### Step 3：写 V{N+1} 补偿迁移

文件名规则与 `coder-migrate-db` 一致：`V{YYYYMMDDHHMMSS}__rollback_<topic>.sql`

```bash
# 生成时间戳
date -u +%Y%m%d%H%M%S
```

### Step 4：本地验证（关键步骤）

```bash
# 启动本地 MySQL
pnpm run dev:infra

# 在 V{N+1} 上跑 docker test 验证
node scripts/run-tests-docker.js <svc>
```

### Step 5：提交 Hotfix PR

```bash
git checkout -b hotfix/rollback-<V{N}>
git add services/<svc>/migrations/
git commit -m "fix(<svc>): rollback V{N} <reason>"
git push -u origin hotfix/rollback-<V{N}>
```

紧急 review 流程：
- 标题加 `[HOTFIX]` 前缀
- 描述包含：回滚原因、备份时间点、验证结果

### Step 6：门禁检查

```bash
node scripts/check-db-naming.js           # 表名前缀规范
node scripts/check-spec.js                # OpenAPI/错误码规范
node scripts/contracts-gen.js --check     # 契约一致性
```

## 预防段

以下写法从一开始就为回滚铺路，优先采用：

### Expand-Contract 模式（推荐）

新增 NOT NULL 字段时，分三步走：

```
Step 1: ADD COLUMN ... NULL           （V{N}）
Step 2: 后台 backfill 数据
Step 3: ALTER ... NOT NULL            （V{N+1}）
```

这样任何一步出错都可以直接 DROP COLUMN 回滚，不丢数据。

### 大批量数据迁移的保险写法

```sql
-- 不要这样（原子性差）：
UPDATE <t> SET col = (SELECT ... FROM other_t);  -- 可能锁全表

-- 应该这样（分批 + 事务）：
START TRANSACTION;
SELECT id FROM <t> WHERE condition INTO OUTFILE '/tmp/batch.csv';
-- 人工确认行数后
LOAD DATA INFILE '/tmp/batch.csv' REPLACE INTO TABLE <t>;
COMMIT;
```

### 创建表前的保险

```sql
-- 建表时加注释，便于日后回滚理解意图
CREATE TABLE <prefix>_<table> (
    id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    ...
) COMMENT 'Phase 5 feature X; rollback: DROP TABLE <prefix>_<table>';
```

## prue_backend SQLite 注意事项

prue_backend 的 SQLite 迁移语法与 MySQL 不同：

- **不支持** `MODIFY` 改类型，需重建表（见上方模板）
- **不支持** `DROP COLUMN`（SQLite 3.35.0+ 才支持，需确认目标环境版本）
- **不支持** `IF EXISTS` / `IF NOT EXISTS` 在所有版本都完整支持
- 索引命名需与 MySQL 版保持一致：`CREATE INDEX IF NOT EXISTS ...`
- **回滚时必须同时写** `migrations/mysql/V{N+1}__*.sql` 和 `migrations/sqlite/V{N+1}__*.sql`

## 快速参考表

| 操作 | 回滚 SQL | 备份必要 |
|------|---------|---------|
| ADD COLUMN | ALTER TABLE ... DROP COLUMN | 建议 |
| CREATE TABLE | DROP TABLE | 必须 |
| DROP COLUMN | 重建列（依赖备份） | 必须 |
| ALTER TYPE | 反向 ALTER | 建议 |
| CREATE INDEX | DROP INDEX | 不需要 |
| DROP INDEX | CREATE INDEX | 不需要 |
| BULK UPDATE | restore + mark_rollback | 必须 |
