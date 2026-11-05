# 生产数据迁移 SOP

> **状态**：草案 · 待评审
> **关联**：ai-context.md §4.2 Onboarding/运维待推进项
> **适用范围**：coder-micro-services 平台所有数据库迁移操作

---

## 0. 一句话

> 所有迁移必须是 forward-only（只新增，从不编辑已发布的迁移文件）。
> 本文档覆盖：备份 → dry-run → 执行 → 验证 → 回滚（fix-forward）全流程。

---

## 1. 核心原则

### 1.1 Forward-Only 原则

```
✅ 正确做法：新增 V003__add_column.sql
❌ 错误做法：编辑已发布的 V001__init.sql 或 V002__add_index.sql
```

**原因**：
- 已发布的迁移可能已在生产环境执行
- 编辑会导致 migration-state 端点返回的状态与实际 DB 状态不一致
- 多人协作时会产生冲突

### 1.2 迁移文件命名

```
V{版本号}__{描述}.sql

示例：
V001__init_users_table.sql
V002__add_email_index.sql
V003__add_tenant_id_column.sql
```

版本号必须是连续整数，从现有最大版本号 +1 开始。

### 1.3 prue_backend 特殊要求

prue_backend 必须同时维护 **MySQL + SQLite 两个版本**：

```
services/prue_backend/
  migrations/
    mysql/
      V001__init.sql
      V002__add_xxx.sql
    sqlite/
      V001__init.sql
      V002__add_xxx.sql
```

**必须同步更新两个版本**，使用 `scripts/check-prue-migrations-parity.sh` 验证 parity。

---

## 2. 迁移前检查清单

### 2.1 代码层面

```bash
# 1. 确认迁移文件位置正确
ls services/<svc>/migrations/mysql/

# 2. 检查命名规范
ls services/<svc>/migrations/mysql/ | grep -E '^V[0-9]+__.*\.sql$'

# 3. 检查表名前缀（Phase 4.0 强制）
node scripts/check-db-naming.js <svc>

# 4. 确认不是编辑已发布迁移（git diff）
git diff services/<svc>/migrations/mysql/
```

### 2.2 表名前缀速查

| 服务前缀 | 表名示例 |
|----------|----------|
| `iam_` | `iam_users`, `iam_roles`, `iam_permissions` |
| `audit_` | `audit_events`, `audit_login_logs` |
| `token_` | `token_oauth_clients`, `token_sessions` |
| `ntf_` | `ntf_announcements`, `ntf_user_settings` |
| `cfg_` | `cfg_namespaces`, `cfg_config_items` |
| `file_` | `file_metadata`, `file_uploads` |
| `prue_` | `prue_menus`, `prue_routes` |
| `pmd_` | `pmd_projects`, `pmd_sprints` |

### 2.3 服务特定检查

```bash
# config-service 已移除（2026-11-06）

# file-service: 检查存储层变更
grep -n "storage" services/file-service/src/

# prue_backend: 验证 MySQL/SQLite parity
bash scripts/check-prue-migrations-parity.sh
```

---

## 3. 备份流程

### 3.1 MySQL 全量备份

```bash
# 备份所有表（不含 views/procs）
docker exec mysql mysqldump \
  -uroot -p"$MYSQL_ROOT_PASSWORD" \
  --single-transaction \
  --routines \
  --triggers \
  --databases superpowers \
  > backups/mysql-superpowers-$(date +%Y%m%d-%H%M%S).sql

# 压缩
gzip backups/mysql-superpowers-$(date +%Y%m%d-%H%M%S).sql

# 验证备份完整性
zcat backups/mysql-superpowers-*.sql.gz | head -100
```

### 3.2 prue_backend SQLite 备份

```bash
# 找到 SQLite 文件位置
docker exec prue_backend ls -la /data/

# 复制到 volume
docker cp prue_backend:/data/prue_backend.db backups/prue_backend-$(date +%Y%m%d).db

# 备份 WAL 和 SHM
docker cp prue_backend:/data/prue_backend.db-wal backups/
docker cp prue_backend:/data/prue_backend.db-shm backups/
```

### 3.3 备份存储

```bash
# 保留策略
# - 每日备份保留 7 天
# - 每周备份保留 4 周
# - 迁移前备份永久保留

# 推荐存储位置
/backups/
  daily/   # 7天滚动
  weekly/  # 4周滚动
  pre-migration/  # 迁移前备份，永久保留
```

---

## 4. Dry-Run（预演）

### 4.1 查看迁移状态

```bash
# 各服务的 migration-state 端点
curl http://localhost:8081/internal/migrations/state  # config-service
curl http://localhost:8082/internal/migrations/state  # token-service
curl http://localhost:8083/internal/migrations/state  # notification-service

# 响应示例
{
  "applied": ["V001", "V002"],
  "pending": ["V003", "V004"],
  "current_version": "V002"
}
```

### 4.2 本地测试

```bash
# 1. 启动本地 MySQL（用备份数据或干净环境）
docker compose -f infrastructure/docker-compose.yml up -d mysql

# 2. 确认当前迁移状态
curl http://localhost:8081/internal/migrations/state

# 3. 手动执行迁移（查看 SQL 是否正确）
# 找到要执行的迁移文件
cat services/<svc>/migrations/mysql/V003__add_column.sql

# 4. 执行迁移（不通过应用，直接 SQL）
docker exec -i mysql mysql -uroot -p"$MYSQL_ROOT_PASSWORD" superpowers < \
  services/<svc>/migrations/mysql/V003__add_column.sql

# 5. 验证
curl http://localhost:8081/internal/migrations/state

# 6. 回滚测试迁移（验证回滚 SQL 存在）
# 注意：我们不执行 down SQL，只是确认它存在
cat services/<svc>/migrations/mysql/V003__add_column.sql
# 检查是否有 -- DOWN 或类似标记
```

---

## 5. 执行流程

### 5.1 步骤

```bash
# 1. 通知团队（Slack/企微）
# /channel #platform
# "开始执行 <svc> 迁移 V003，预计耗时 5 分钟"

# 2. 确认备份已完成
ls -la backups/pre-migration/

# 3. 执行迁移
# 应用迁移由服务启动时自动执行
# 但如果是独立 migration runner：
docker compose -f infrastructure/docker-compose-services.yml restart <svc>

# 4. 观察日志
docker logs -f <svc> --tail 100

# 5. 验证迁移状态
curl http://localhost:8081/internal/migrations/state

# 6. 健康检查
curl http://localhost:8081/health

# 7. 冒烟测试
curl -X POST http://localhost:3000/v1/iam/health
curl -X POST http://localhost:3000/v1/<svc>/health
```

### 5.2 迁移成功标志

```
✅ 迁移状态端点返回 pending 为空
✅ 服务健康检查返回 200
✅ 日志无 migration 错误
✅ 冒烟测试通过
```

---

## 6. 回滚流程（Fix-Forward）

> **重要**：不执行 down 迁移，而是写一个新的 forward 迁移来修复问题。

### 6.1 为什么是 Fix-Forward

```
原因：
1. 已执行的部分 DDL（如 ALTER TABLE）无法自动回滚
2. 回滚 SQL 可能破坏数据
3. forward-only 保证了迁移历史清晰
```

### 6.2 Fix-Forward 步骤

```bash
# 1. 诊断问题
docker logs <svc> | grep -i "migration\|error\|failed"

# 2. 写修复迁移（新版本，不是编辑旧版本）
# 例如：如果 V003 漏了一个索引
echo "-- V004__fix_missing_index.sql
CREATE INDEX idx_users_email ON iam_users(email);
" >> services/iam-service/migrations/mysql/V004__fix_missing_index.sql

# 3. 提交修复
git add services/iam-service/migrations/mysql/V004__fix_missing_index.sql
git commit -m "fix(iam): add missing email index in V004"

# 4. 执行新迁移
docker compose -f infrastructure/docker-compose-services.yml restart iam-service

# 5. 验证
curl http://localhost:8082/internal/migrations/state
```

### 6.3 数据修复迁移

如果迁移导致数据问题，需要写数据修复迁移：

```sql
-- V005__fix_invalid_tenant_data.sql
-- 修复 V003 迁移后产生的无效 tenant_id 数据
UPDATE iam_users
SET tenant_id = 'default'
WHERE tenant_id IS NULL OR tenant_id = '';
```

---

## 7. 大表迁移（零停机）

### 7.1 原则

```
阶段1：添加 nullable 列或索引（不锁表）
阶段2：后台批量回填数据
阶段3：添加 NOT NULL 约束或唯一索引
```

### 7.2 示例：三阶段添加 tenant_id

```sql
-- V003：添加 nullable 列（立即完成，不锁表）
ALTER TABLE iam_users ADD COLUMN tenant_id VARCHAR(64) NULL;

-- V004：后台回填（分批，异步）
-- 应用代码中后台任务执行：
UPDATE iam_users SET tenant_id = 'default' WHERE id BETWEEN 1 AND 1000;
UPDATE iam_users SET tenant_id = 'default' WHERE id BETWEEN 1001 AND 2000;
-- ... 重复直到全部更新

-- V005：添加约束（数据已干净，可以执行）
ALTER TABLE iam_users MODIFY COLUMN tenant_id VARCHAR(64) NOT NULL DEFAULT 'default';
```

---

## 8. 迁移后验证

### 8.1 检查清单

```bash
# 1. 迁移状态
curl http://localhost:8081/internal/migrations/state
# 确认 applied 包含新版本

# 2. 表结构
docker exec mysql mysql -uroot -p"$MYSQL_ROOT_PASSWORD" -e \
  "DESCRIBE superpowers.<table_name>;"

# 3. 索引
docker exec mysql mysql -uroot -p"$MYSQL_ROOT_PASSWORD" -e \
  "SHOW INDEX FROM superpowers.<table_name>;"

# 4. 数据完整性
docker exec mysql mysql -uroot -p"$MYSQL_ROOT_PASSWORD" -e \
  "SELECT COUNT(*) FROM superpowers.<table_name> WHERE <problem_condition>;"

# 5. E2E 冒烟测试
pnpm -C tests/e2e test:smoke
```

### 8.2 常见错误处理

| 错误 | 原因 | 处理 |
|------|------|------|
| `Duplicate key` | 迁移重复执行 | 幂等化（IF NOT EXISTS） |
| `Lock wait timeout` | 大表 ALTER | 分批执行或 pt-online-schema-change |
| `Column not found` | 迁移顺序错误 | 检查 applied 列表 |
| `Data too long` | 字段长度不足 | 新增迁移加宽字段 |

---

## 9. 关联

- `docs-site/guides/migrations-state.md` — migration-state 端点
- `scripts/check-db-naming.js` — 表名检查
- `scripts/check-prue-migrations-parity.sh` — prue_backend parity 检查
- `docs-site/runbooks/troubleshooting.md` — 迁移故障排查

---

> **最后更新**：2026-07-13
