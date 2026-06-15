# coder-micro-services AI 协作指南

## Skill 索引

| Skill | 触发关键词 | 命令 |
|---|---|---|
| `coder-run-tests` | 测试、test | node scripts/test-orchestrator.js loop |
| `coder-base-images-sync` | base images、镜像清单 | node scripts/base-images-sync.js |
| `coder-route-validate` | 校验路由、reload | cargo run -p api-gateway -- validate |
| `coder-spec-to-code` | spec-to-code、写测试用例 | node scripts/spec-to-code.js |
| `coder-sdk-publish` | 发 SDK、sdk:publish | node scripts/sdk-publish.js |
| `coder-microservice-dev` | 起服务、本地开发 | node scripts/dev-services.js |
| `coder-fix-tests` | 修测试、fix-prompt | node scripts/test-orchestrator.js fix |
| `coder-service-standard-check` | 检查服务规范、standard | node scripts/check-service-standard.js |
| `coder-e2e-test` | E2E测试、登录验证 | node scripts/run-e2e.js |
| `coder-lib-upgrade` | 升级通用库 | cargo upgrade -p <lib> |
| `coder-service-audit` | 服务审计、规范检查 | node scripts/check-service-standard.js --service <name> |

## 服务规范规则系统

### 概述

每个服务目录下 `AGENTS.md` 定义服务开发规范。

### 规范内容

- Dockerfile
- Swagger 配置（环境变量控制）
- 健康检查端点
- 数据库迁移文件
- README.md
- tests/ 目录

### AI 检查时机

修改 `services/<name>/` 下文件时自动检查规范。

### 环境变量配置

- `SWAGGER_ENABLED`: 是否启用 Swagger
- `HEALTH_CHECK_REQUIRED`: 是否要求健康检查
- `MIGRATIONS_REQUIRED`: 是否要求迁移文件

## 契约层

### 概述

`contracts/` 目录是前后端一致的单一真相源。

### 文件结构

| 文件 | 用途 |
|---|---|
| error-codes.yaml | 全局错误码字典 |
| permissions.yaml | 权限码字典 |
| events.yaml | 领域事件名（预留） |
| http-conventions.md | HTTP 响应/分页/排序约定 |

### 代码生成

```bash
node scripts/contracts-gen.js
```

生成目标：
- `libs/common-error/src/codes.rs` — Rust 错误码常量
- `libs/authz/src/perms.rs` — Rust 权限码常量
- `frontend/src/contracts/*.ts` — TypeScript 类型（预留）

### CI 校验

```bash
node scripts/test-orchestrator.js validate-contracts
```

检查生成的代码是否与 YAML 源文件一致，防止漂移。