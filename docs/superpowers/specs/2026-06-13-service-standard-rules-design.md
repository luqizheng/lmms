# 服务规范规则系统设计

> AI 可读并自动加载的服务开发规范

---

## 概述

**目标**：为 `services/` 目录下的微服务定义 AI 可读的规范规则，确保服务开发符合统一标准。

**核心需求**：
- AI 在修改服务文件时自动检查规范
- 支持环境变量配置（Swagger 启用/禁用等）
- 提供必需组件检查（Dockerfile、Swagger、健康检查等）
- 统一的 package.json 构建脚本

---

## 1. 规则文件结构

### 1.1 文件位置

每个服务目录下创建规则文件：
```
services/<name>/AGENTS.md
```

### 1.2 文件格式

```markdown
# <service-name> 服务规范

## 必需组件

- Dockerfile
- Swagger 配置（可配置启用/禁用）
- 健康检查端点（/health、/ready、/metrics）
- 数据库迁移文件（migrations/）
- README.md
- tests/ 目录

## 配置选项

- swagger_enabled: 环境变量 SWAGGER_ENABLED 或默认 true
- health_check_required: 环境变量 HEALTH_CHECK_REQUIRED 或默认 true
- migrations_required: 环境变量 MIGRATIONS_REQUIRED 或默认 true

## package.json 脚本

- build:docker
- build:cargo

## AI 检查时机

修改 services/<name>/ 下文件时自动检查
```

---

## 2. AI 自动检查机制

### 2.1 触发时机

- AI 修改 `services/<name>/src/**/*.rs` 时
- AI 修改 `services/<name>/Cargo.toml` 时
- AI 修改 `services/<name>/config/**` 时

### 2.2 检查流程

1. AI 读取 `services/<name>/AGENTS.md`
2. 解析必需组件列表
3. 检查服务目录是否包含必需文件
4. 读取环境变量配置（如 `SWAGGER_ENABLED`）
5. 缺失组件时提醒用户

### 2.3 提醒格式

```
⚠️ 服务规范检查：api-gateway 缺失以下组件：
- Swagger 配置（SWAGGER_ENABLED=true）
- 健康检查端点 /metrics

建议：添加 swagger.rs 和 metrics.rs
```

---

## 3. 具体规范内容定义

### 3.1 Dockerfile 规范

- **必须存在**：`services/<name>/Dockerfile`
- **必须包含**：多阶段构建、最小化镜像、健康检查指令
- **AI 检查**：文件是否存在，是否包含 `HEALTHCHECK` 指令

### 3.2 Swagger 配置规范

- **配置文件**：`services/<name>/config/swagger.yaml` 或 Cargo.toml metadata
- **环境变量控制**：`SWAGGER_ENABLED=true/false`
- **AI 检查**：配置文件是否存在，环境变量是否设置
- **启用时**：必须提供 `/swagger` 或 `/docs` 端点

### 3.3 健康检查端点规范

- **必须提供**：
  - `/health`：服务状态
  - `/ready`：依赖就绪
  - `/metrics`：Prometheus 指标
- **环境变量控制**：`HEALTH_CHECK_REQUIRED=true/false`
- **AI 检查**：路由文件是否包含健康检查路由

### 3.4 数据库迁移规范

- **必须存在**：`services/<name>/migrations/` 目录
- **文件格式**：`V001__init.sql`、`V002__add_table.sql`（版本化）
- **环境变量控制**：`MIGRATIONS_REQUIRED=true/false`
- **AI 检查**：目录是否存在，文件命名是否规范

### 3.5 README 规范

- **必须存在**：`services/<name>/README.md`
- **必须包含**：服务职责、API 端点、配置说明、依赖列表
- **AI 检查**：文件是否存在，是否包含必要章节

### 3.6 测试规范

- **必须存在**：`services/<name>/tests/` 目录
- **必须包含**：`integration_test.rs`
- **AI 检查**：目录是否存在，测试文件是否存在

---

## 4. package.json 脚本定义

### 4.1 根目录 package.json 脚本

```json
{
  "scripts": {
    "build:docker": "node scripts/build-docker.js",
    "build:cargo": "node scripts/build-cargo.js",
    "check:service-standard": "node scripts/check-service-standard.js"
  }
}
```

### 4.2 build-docker.js 功能

- 遍历 `services/` 目录
- 为每个服务执行 `docker build -t <name>:latest`
- 检查 Dockerfile 是否存在，缺失时报错

### 4.3 build-cargo.js 功能

- 遍历 `services/` 目录
- 为每个 Rust 服务执行 `cargo build --release`
- 检查 Cargo.toml 是否存在

### 4.4 check-service-standard.js 功能

- 遍历 `services/` 目录
- 读取每个服务的 `AGENTS.md`
- 检查必需组件是否存在
- 输出检查报告

---

## 5. 实施步骤

### 阶段 1：创建规则文件模板

- 创建 `services/agents-template.md`（模板文件）
- 为现有服务创建规则文件：
  - `services/api-gateway/AGENTS.md`
  - `services/token-service/AGENTS.md`
  - `services/notification-service/AGENTS.md`
  - `services/iam-service/AGENTS.md`

### 阶段 2：创建检查脚本

- 创建 `scripts/check-service-standard.js`
- 创建 `scripts/build-docker.js`
- 创建 `scripts/build-cargo.js`
- 更新根目录 `package.json` 添加脚本

### 阶段 3：集成 AI 规则

- 更新 `.trae/rules/project_rules.md`，添加服务规范检查触发规则
- 定义 AI 在修改服务文件时的自动检查行为

### 阶段 4：验证和测试

- 运行 `npm run check:service-standard` 验证现有服务
- 测试 AI 自动检查功能
- 补充缺失组件（如 Swagger 配置、健康检查端点）

### 阶段 5：文档和培训

- 更新 `docs/guides/` 添加服务规范指南
- 更新 `AGENTS.md` 说明新规则系统

---

## 6. 环境变量配置

### 6.1 支持的环境变量

| 环境变量 | 默认值 | 说明 |
|---------|--------|------|
| `SWAGGER_ENABLED` | `true` | 是否启用 Swagger 文档 |
| `HEALTH_CHECK_REQUIRED` | `true` | 是否要求健康检查端点 |
| `MIGRATIONS_REQUIRED` | `true` | 是否要求数据库迁移文件 |

### 6.2 环境变量读取逻辑

- 优先读取环境变量
- 未设置时使用默认值
- AI 检查时同时检查环境变量和实际组件

---

## 7. 与现有系统集成

### 7.1 与 project_rules.md 集成

在 `.trae/rules/project_rules.md` 中添加：

```markdown
## 路径 → Skill 自动匹配

| 改动路径 | 建议 Skill | 时机 |
|---------|-----------|------|
| `services/*/AGENTS.md` | 自动检查服务规范 | 新建/修改 |
| `services/*/src/**/*.rs` | 检查服务规范 + coder-run-tests | 改完保存 |
```

### 7.2 与 skills 系统集成

可选：创建新 skill `coder-service-standard-check`，统一管理服务规范检查逻辑。

---

## 8. 成功标准

- 所有现有服务都有 `AGENTS.md` 文件
- `npm run check:service-standard` 能正确检查所有服务
- AI 在修改服务文件时能自动提醒缺失组件
- 环境变量配置能动态调整规范要求
- package.json 脚本能统一构建所有服务

---

## 9. 风险和缓解

### 9.1 风险：规则文件维护成本

- **缓解**：提供模板文件，新服务复制模板即可
- **缓解**：规则文件内容简单，易于维护

### 9.2 风险：AI 检查误报

- **缓解**：环境变量控制，允许服务禁用某些规范
- **缓解**：提醒格式清晰，用户可判断是否需要补充

### 9.3 风险：现有服务缺失组件

- **缓解**：分阶段实施，先创建规则文件，再补充组件
- **缓解**：检查脚本输出详细报告，指导补充方向

---

## 10. 未来扩展

- 支持更多规范（如日志格式、错误处理标准）
- 支持服务间依赖检查
- 支持自动生成缺失组件（如自动生成 README）
- 支持规范版本管理（如 v1、v2 规范）