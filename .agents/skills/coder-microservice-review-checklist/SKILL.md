---
name: coder-microservice-review-checklist
description: |
  Repo-specific mandatory PR checklist for reviewing Rust microservice changes in
  coder-micro-services. Triggers on "review PR", "code review", "审查", "PR 检查",
  "合规检查", "服务规范", "X-User-*", "outbox", "表名前缀", "trans 审计",
  or when review-work skill is invoked on services/** changes.
---

# coder-microservice-review-checklist

对 services/ 下 Rust 微服务 PR/变更进行合规审查的强制清单。叠加于 `review-work` 通用 review，不替代它。

## 何时用

用户说「review」「审查」「PR 检查」「合规检查」且变更触及 `services/` 任一子目录时调用。变更含 `migrations/` 时优先叠加 `coder-migrate-db` 检查。

## 前置

- 工作目录：项目根目录
- Node.js 18+（跑 `node scripts/` 检查脚本）
- Rust 工具链（`cargo`，本地验证编译时用）

## 必查项 Top N（按出现频率排序）

每项含 grep 检测命令 + 修复方向。grep 返回非空即违规。

### 1. 禁止使用 X-User-* Headers（安全红线）

**规则**：下游服务必须从 `Authorization: Bearer <jwt>` 解码 `Principal`，禁止读取 `X-User-Id`/`X-User-Roles` 等 headers。网关已验签，下游不重复验签。

```bash
# 违规检测：services/ 下任意 X-User- 出现
rg "X-User-" services/ --type rust
```

**修复**：改用 `digua_jwt_ctx::Principal` extractor 从 JWT payload 取身份。

---

### 2. 表名前缀必须为 iam_/audit_/token_/ntf_/cfg_/file_/prue_

**规则**：共享 MySQL `superpowers` 库，表名撞名会导致静默数据丢失。sqlx 字符串内表名也须带前缀。

```bash
# 违规检测：CREATE TABLE 无前缀
rg "CREATE TABLE (?!iam_|audit_|token_|ntf_|cfg_|file_|prue_)" services/ --type sql -l

# 违规检测：Rust sqlx 字符串 FROM/JOIN/INTO/UPDATE/DELETE FROM 后无前缀表名
rg 'FROM|JOIN|INTO|UPDATE|DELETE FROM' services/ --type rust | rg -v 'iam_|audit_|token_|ntf_|cfg_|file_|prue_'
```

**修复**：表名加服务前缀，sqlx 字符串中的表名同步修正。CI `node scripts/check-db-naming.js` 会拦截。

---

### 3. 错误码必须先注册后引用

**规则**：错误码常量在 `contracts/error-codes.yaml` 注册后由 `node scripts/contracts-gen.js` 生成到 `libs/digua_common-error/src/codes.rs`，Rust 代码必须 import 生成常量，禁止硬编码字符串。

```bash
# 违规检测：硬编码错误码字符串（不含生成常量路径）
rg '"(IAM|AUD|NTF|CFG|FILE|GW|SYS)_[A-Z]+_[0-9]+"' services/ --type rust | rg -v 'use.*codes::'
```

**修复**：在 `contracts/error-codes.yaml` 添加条目，跑 `node scripts/contracts-gen.js`，Rust 代码 import 对应常量。

---

### 4. 权限码必须先注册后引用

**规则**：权限码在 `contracts/permissions.yaml` 注册后由 `contracts-gen.js` 生成到 `libs/digua_authz/src/perms.rs`。

```bash
# 违规检测：硬编码权限码字符串（不含 perms 模块路径）
rg '"(IAM|AUD|NTF|CFG|FILE|GW)_[A-Z]+"services/ --type rust | rg -v 'use.*perms::'
```

**修复**：在 `contracts/permissions.yaml` 添加条目，跑 `node scripts/contracts-gen.js`，Rust 代码 import 生成常量。

---

### 5. 跨服务写操作必须走 RabbitMQ + Outbox

**规则**：跨服务写库禁止直接 HTTP 调下游，必须用 transactional outbox（`digua_common_event::MySqlOutboxRepo::enqueue`）+ RabbitMQ。

```bash
# 违规检测：直接 HTTP POST/PUT/DELETE 到下游服务写库
rg "http://(token|iam|audit|config|file|notification)-service" services/ --type rust | rg -v "consul|jwks|.well-known"
```

**修复**：改用 `MySqlOutboxRepo::enqueue` 入 outbox 事件，独立的 `*-outbox-publisher` 进程轮询发布到 RabbitMQ。

---

### 6. openapi.yaml 必须随路由改动同 commit

**规则**：`services/*/openapi.yaml` 是 utoipa 自动生成的，禁止直接编辑。改路由后跑 `node scripts/dump-openapi-specs.js` 重新生成，与代码改动同 commit。

```bash
# 违规检测：变更了路由但 openapi.yaml 未变（diff 中无 openapi.yaml）
git diff --name-only HEAD | rg "^services/.*\.rs$" && git diff --name-only HEAD | rg "openapi.yaml" || echo "openapi.yaml missing"
```

**修复**：改路由后跑 `node scripts/dump-openapi-specs.js`，同 commit 提交生成的 yaml 文件。CI `git diff --exit-code` 会拦截 drift。

---

### 7. Swagger 走 SWAGGER_ENABLED 环境变量

**规则**：硬编码 `false` 会让 CI 的 Swagger UI 检查失败。

```bash
# 违规检测：Swagger 配置硬编码 false
rg "SWAGGER_ENABLED\s*=\s*false" services/ --type rust
```

**修复**：删除硬编码值，默认 `true`（本地开发友好），生产通过环境变量覆盖。

---

### 8. /health 和 /metrics 端点必须存在

**规则**：健康检查和 Prometheus 指标端点是服务规范的强制要求。

```bash
# 违规检测：未注册 /health 或 /metrics
rg "/health|/metrics" services/*/src/ --type rust -l | wc -l
# 若数量 < 服务数，说明有服务缺端点
```

**修复**：在路由注册中添加这两个端点。

---

### 9. Commit Message 格式

**规则**：`<Phase>:<summary>` 或 `feat(<scope>): <summary>`。

```bash
# 违规检测：非标准 commit message
git log --oneline -10 | rg -v "^[A-Z][a-z]+(\([a-z0-9_-]+\))?: ."
```

**修复**：按格式重写 commit message，禁止 amend，走新 commit。

---

### 10. Migration 是 Forward-Only

**规则**：已发布迁移禁止编辑。新增 `V{N+1}__*.sql`。

```bash
# 违规检测：修改已存在迁移文件
git diff HEAD -- 'services/*/migrations/**/*.sql' | rg "^-(CREATE|ALTER|DROP)" | rg -v "^-- Add"
```

**修复**：新建迁移文件，不要编辑历史迁移。

---

### 11. 服务端口约定

**规则**：服务内部端口统一 `8080`；对外端口范围 `6000-6999`（token-service=6001, iam-service=6002, notification-service=6003, prue_backend=6004, audit-service=6005, config-service=6006, file-service=6007）；api-gateway 对外/对内均为 `3000`。

```bash
# 违规检测：硬编码端口号（不在 8080/3000 且非 6000-6999 范围）
rg "Port\s*\(\s*env::var\(\"PORT|port\s*=\s*[0-9]+" services/ --type rust
```

**修复**：硬编码端口改为默认 8080，新服务不要占用 6000-6999 之外的端口。

---

### 12. Cargo [[bin]] name → 容器二进制名下划线映射

**规则**：`[[bin]] name = "kebab-case"` 在 cargo build 后强制映射为 `/usr/local/bin/snake_case`（kebab → underscore）；`infrastructure/docker-compose-services.yml` 的 `command:` 字段必须用**下划线名**。把 kebab-case 误抄进 compose 会导致 OCI runtime 在 `$PATH` 找不到 binary，容器创建失败。

```bash
# 违规检测：kebab-case bin 名
rg "name\s*=\s*\"[a-z]+-[a-z]" services/*/Cargo.toml

# 违规检测：compose 里 command: 仍写 kebab-case（应全小写下划线）
rg "command:.*\[\".*-.*\"\]" infrastructure/docker-compose-services.yml
```

**修复**：compose `command:` 必须与 cargo build 产出的下划线二进制名一致。`name = "ntf-outbox-publisher"` → `/usr/local/bin/ntf_outbox_publisher`。

---

## 安全类必查

| # | 检查项 | 检测命令 | 修复方向 |
|---|--------|----------|----------|
| S1 | X-User-* headers 禁用 | `rg "X-User-" services/ --type rust` | 用 `Principal` extractor |
| S2 | SQL 注入风险（字符串拼接） | `rg "format!\(.*SELECT|INSERT|UPDATE|DELETE" services/ --type rust` | 用 sqlx 参数化查询 |
| S3 | 硬编码密钥/密码 | `rg "password\s*=\s*[\"'][^\"']+[\"']" services/ --type rust` | 移至 env 或 config.yaml |
| S4 | 缺少鉴权（公开接口但应有 auth） | `rg "axum::routing::get.*without_auth\|allow_anonymous" services/ --type rust` | 确认是否故意省略 |

## 契约类必查

| # | 检查项 | 检测命令 | 修复方向 |
|---|--------|----------|----------|
| C1 | 错误码先注册后引用 | `rg '"(IAM|AUD|NTF|CFG)_[A-Z]+_[0-9]+"' services/ --type rust \| rg -v use` | 注册到 error-codes.yaml |
| C2 | 权限码先注册后引用 | `rg '"(IAM|AUD|NTF|CFG)_[A-Z]+"services/ --type rust \| rg -v use` | 注册到 permissions.yaml |
| C3 | openapi.yaml drift | `git diff HEAD -- 'services/*/openapi.yaml'` | 跑 dump-openapi-specs.js |

## 数据库类必查

| # | 检查项 | 检测命令 | 修复方向 |
|---|--------|----------|----------|
| D1 | Migration 表名前缀 | `rg "CREATE TABLE (?!iam_\|audit_\|token_\|ntf_\|cfg_\|file_\|prue_)" services/ --type sql -l` | 加服务前缀 |
| D2 | sqlx 字符串表名前缀 | `rg 'FROM\|JOIN\|INTO\|UPDATE\|DELETE FROM' services/ --type rust \| rg -v iam_\|audit_\|token_\|ntf_\|cfg_\|file_\|prue_` | 字符串内表名加前缀 |
| D3 | Forward-only 迁移 | `git diff HEAD -- 'services/*/migrations/**/*.sql' \| rg "^-" \| rg -v "^--"` | 禁改已有迁移 |

## 跨服务通信类必查

| # | 检查项 | 检测命令 | 修复方向 |
|---|--------|----------|----------|
| M1 | 直接 HTTP 调下游（应走 outbox） | `rg "http://(token\|iam\|audit\|config\|file\|notification)-service" services/ --type rust \| rg -v consul\|jwks` | 改用 outbox + RabbitMQ |
| M2 | Outbox 表写入存在 | `rg "digua_common_event\|MySqlOutboxRepo::enqueue" services/ --type rust -l` | 生产者侧必须有此调用 |

## 性能/资源类必查

| # | 检查项 | 检测命令 | 修复方向 |
|---|--------|----------|----------|
| P1 | /health 端点缺失 | `rg "/health" services/*/src/ --type rust -l \| wc -l` | 添加健康检查 handler |
| P2 | /metrics 端点缺失 | `rg "/metrics" services/*/src/ --type rust -l \| wc -l` | 添加 Prometheus metrics handler |
| P3 | 大数组 clone（性能隐患） | `rg "\.clone\(\)\.collect::<Vec" services/ --type rust` | 用引用或迭代器代替 |
| P4 | 同步阻塞调用（应异步） | `rg "std::thread::sleep\|tokio::runtime::Runtime::new" services/ --type rust` | 确认是否故意阻塞 |

## 验证

所有 CI gate 本地复刻命令：

```bash
# 必须全过才能 push
node scripts/check-service-standard.js
node scripts/check-spec.js
node scripts/contracts-gen.js --check
node scripts/check-db-naming.js
git diff --exit-code -- 'services/*/openapi.yaml'
```

## 快速一键检查

复制以下脚本，对 PR 变更文件做机械筛查（返回空 = 通过，非空 = 违规）：

```bash
#!/usr/bin/env bash
set -e
cd "$(git rev-parse --show-toplevel)"

echo "=== 1. X-User-* headers 禁用 ==="
rg "X-User-" services/ --type rust || true

echo "=== 2. 表名前缀（SQL）==="
rg "CREATE TABLE (?!iam_|audit_|token_|ntf_|cfg_|file_|prue_)" services/ --type sql -l || true

echo "=== 3. 表名前缀（Rust sqlx 字符串）==="
rg 'FROM |JOIN |INTO |UPDATE |DELETE FROM' services/ --type rust | rg -v 'iam_|audit_|token_|ntf_|cfg_|file_|prue_' || true

echo "=== 4. 硬编码错误码字符串 ==="
rg '"(IAM|AUD|NTF|CFG|FILE|GW|SYS)_[A-Z]+_[0-9]+"' services/ --type rust | rg -v 'use.*codes::' || true

echo "=== 5. 直接 HTTP 调下游写库 ==="
rg "http://(token|iam|audit|config|file|notification)-service" services/ --type rust | rg -v "consul|jwks|.well-known" || true

echo "=== 6. Swagger 硬编码 false ==="
rg "SWAGGER_ENABLED\s*=\s*false" services/ --type rust || true

echo "=== 7. 硬编码密码/密钥 ==="
rg "password\s*=\s*[\"'][^\"']{4,}[\"']" services/ --type rust || true

echo "=== 8. Migration 是否改旧文件 ==="
git diff HEAD -- 'services/*/migrations/**/*.sql' | rg "^-" | rg -v "^---" || true

echo "=== 9. openapi.yaml drift ==="
git diff HEAD -- 'services/*/openapi.yaml' | head -20 || true

echo "=== 10. Commit message 格式（最近 5 条）==="
git log --oneline -5 | rg -v "^[A-Z][a-z]+(\([a-z0-9_-]+\))?: " || true
```

## 如何用本 Skill

本 skill 与 `review-work`（通用代码审查）的关系是**叠加而非替代**：

- `review-work`：通用代码质量（变量命名、死代码、逻辑错误、测试覆盖）
- **本 skill**：repo 特定的强制合规规则（身份认证、契约注册、表名前缀、outbox 模式、CI gate 拦截项）

使用顺序：先跑 `review-work`，再跑本清单。两份检查都通过才视为完成 review。

**注意**：本清单只做机械筛查，不做设计层面评审。架构问题走 `plan-eng-review` skill。
