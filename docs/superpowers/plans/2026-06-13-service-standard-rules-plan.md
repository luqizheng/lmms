# 服务规范规则系统实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 services/ 目录下的微服务创建 AI 可读的规范规则系统，确保服务开发符合统一标准。

**Architecture:** 在每个服务目录下创建 `agents.md` 规则文件，AI 修改服务文件时自动检查规范。创建 Node.js 检查脚本，集成到 package.json 和 project_rules.md。

**Tech Stack:** Node.js (scripts), Markdown (rules), YAML (config), Rust (services)

---

## Task 1: 创建规则文件模板

**Files:**
- Create: `services/agents-template.md`

- [ ] **Step 1: 创建模板文件**

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

- [ ] **Step 2: 验证模板文件存在**

Run: `Get-Content services\agents-template.md`
Expected: 显示模板内容

- [ ] **Step 3: 提交模板文件**

```bash
git add services/agents-template.md
git commit -m "chore(ai): add service agents template"
```

---

## Task 2: 为 api-gateway 创建规则文件

**Files:**
- Create: `services/api-gateway/agents.md`

- [ ] **Step 1: 创建 api-gateway 规则文件**

```markdown
# api-gateway 服务规范

## 必需组件

- Dockerfile ✓
- Swagger 配置（可配置启用/禁用）⚠️ 待补充
- 健康检查端点（/health、/ready、/metrics）⚠️ 待补充 /metrics
- 数据库迁移文件（migrations/）✗ 不适用（无数据库）
- README.md ✓
- tests/ 目录 ✓

## 配置选项

- swagger_enabled: 环境变量 SWAGGER_ENABLED 或默认 true
- health_check_required: 环境变量 HEALTH_CHECK_REQUIRED 或默认 true
- migrations_required: 环境变量 MIGRATIONS_REQUIRED 或默认 false（网关无数据库）

## package.json 脚本

- build:docker
- build:cargo

## AI 检查时机

修改 services/api-gateway/ 下文件时自动检查
```

- [ ] **Step 2: 验证规则文件存在**

Run: `Get-Content services\api-gateway\agents.md`
Expected: 显示规则内容

- [ ] **Step 3: 提交规则文件**

```bash
git add services/api-gateway/agents.md
git commit -m "feat(api-gateway): add agents file"
```

---

## Task 3: 为 token-service 创建规则文件

**Files:**
- Create: `services/token-service/agents.md`

- [ ] **Step 1: 创建 token-service 规则文件**

```markdown
# token-service 服务规范

## 必需组件

- Dockerfile ✓
- Swagger 配置（可配置启用/禁用）⚠️ 待补充
- 健康检查端点（/health、/ready、/metrics）⚠️ 待补充 /metrics
- 数据库迁移文件（migrations/）✓
- README.md ✓
- tests/ 目录 ✓

## 配置选项

- swagger_enabled: 环境变量 SWAGGER_ENABLED 或默认 true
- health_check_required: 环境变量 HEALTH_CHECK_REQUIRED 或默认 true
- migrations_required: 环境变量 MIGRATIONS_REQUIRED 或默认 true

## package.json 脚本

- build:docker
- build:cargo

## AI 检查时机

修改 services/token-service/ 下文件时自动检查
```

- [ ] **Step 2: 验证规则文件存在**

Run: `Get-Content services\token-service\agents.md`
Expected: 显示规则内容

- [ ] **Step 3: 提交规则文件**

```bash
git add services/token-service/agents.md
git commit -m "feat(token-service): add agents file"
```

---

## Task 4: 为 notification-service 创建规则文件

**Files:**
- Create: `services/notification-service/agents.md`

- [ ] **Step 1: 创建 notification-service 规则文件**

```markdown
# notification-service 服务规范

## 必需组件

- Dockerfile ✓
- Swagger 配置（可配置启用/禁用）⚠️ 待补充
- 健康检查端点（/health、/ready、/metrics）⚠️ 待补充 /metrics
- 数据库迁移文件（migrations/）✓
- README.md ✓
- tests/ 目录 ✓

## 配置选项

- swagger_enabled: 环境变量 SWAGGER_ENABLED 或默认 true
- health_check_required: 环境变量 HEALTH_CHECK_REQUIRED 或默认 true
- migrations_required: 环境变量 MIGRATIONS_REQUIRED 或默认 true

## package.json 脚本

- build:docker
- build:cargo

## AI 检查时机

修改 services/notification-service/ 下文件时自动检查
```

- [ ] **Step 2: 验证规则文件存在**

Run: `Get-Content services\notification-service\agents.md`
Expected: 显示规则内容

- [ ] **Step 3: 提交规则文件**

```bash
git add services/notification-service/agents.md
git commit -m "feat(notification-service): add agents file"
```

---

## Task 5: 为 iam-service 创建规则文件

**Files:**
- Create: `services/iam-service/agents.md`

- [ ] **Step 1: 创建 iam-service 规则文件**

```markdown
# iam-service 服务规范

## 必需组件

- Dockerfile ✓
- Swagger 配置（可配置启用/禁用）⚠️ 待补充
- 健康检查端点（/health、/ready、/metrics）⚠️ 待补充 /metrics
- 数据库迁移文件（migrations/）✓
- README.md ✓
- tests/ 目录 ✓

## 配置选项

- swagger_enabled: 环境变量 SWAGGER_ENABLED 或默认 true
- health_check_required: 环境变量 HEALTH_CHECK_REQUIRED 或默认 true
- migrations_required: 环境变量 MIGRATIONS_REQUIRED 或默认 true

## package.json 脚本

- build:docker
- build:cargo

## AI 检查时机

修改 services/iam-service/ 下文件时自动检查
```

- [ ] **Step 2: 验证规则文件存在**

Run: `Get-Content services\iam-service\agents.md`
Expected: 显示规则内容

- [ ] **Step 3: 提交规则文件**

```bash
git add services/iam-service/agents.md
git commit -m "feat(iam-service): add agents file"
```

---

## Task 6: 创建服务规范检查脚本

**Files:**
- Create: `scripts/check-service-standard.js`

- [ ] **Step 1: 创建检查脚本框架**

```javascript
const fs = require('node:fs');
const path = require('node:path');

const SERVICES_DIR = path.join(__dirname, '..', 'services');

function checkServiceStandard() {
  const services = fs.readdirSync(SERVICES_DIR, { withFileTypes: true })
    .filter(dirent => dirent.isDirectory())
    .map(dirent => dirent.name);

  console.log('=== 服务规范检查报告 ===\n');

  for (const serviceName of services) {
    const serviceDir = path.join(SERVICES_DIR, serviceName);
    const rulesFile = path.join(serviceDir, 'agents.md');

    console.log(`\n## ${serviceName}`);

    if (!fs.existsSync(rulesFile)) {
      console.log('⚠️  缺失规则文件: agents.md');
      continue;
    }

    const rulesContent = fs.readFileSync(rulesFile, 'utf-8');
    const requiredComponents = parseRequiredComponents(rulesContent);

    for (const component of requiredComponents) {
      const componentPath = getComponentPath(serviceDir, component);
      const exists = fs.existsSync(componentPath);

      if (exists) {
        console.log(`✓ ${component}`);
      } else {
        console.log(`⚠️  缺失: ${component}`);
      }
    }
  }
}

function parseRequiredComponents(content) {
  const match = content.match(/## 必需组件\n([\s\S]*?)\n##/);
  if (!match) return [];

  return match[1]
    .split('\n')
    .filter(line => line.startsWith('-'))
    .map(line => line.replace('- ', '').trim());
}

function getComponentPath(serviceDir, component) {
  const componentMap = {
    'Dockerfile': 'Dockerfile',
    'Swagger 配置': 'config/swagger.yaml',
    '健康检查端点': 'src/health.rs',
    '数据库迁移文件': 'migrations',
    'README.md': 'README.md',
    'tests/ 目录': 'tests'
  };

  return path.join(serviceDir, componentMap[component] || component);
}

checkServiceStandard();
```

- [ ] **Step 2: 验证脚本存在**

Run: `Get-Content scripts\check-service-standard.js`
Expected: 显示脚本内容

- [ ] **Step 3: 测试脚本运行**

Run: `node scripts/check-service-standard.js`
Expected: 显示检查报告

- [ ] **Step 4: 提交脚本**

```bash
git add scripts/check-service-standard.js
git commit -m "feat(scripts): add service standard check script"
```

---

## Task 7: 创建 Docker 构建脚本

**Files:**
- Create: `scripts/build-docker.js`

- [ ] **Step 1: 创建 Docker 构建脚本**

```javascript
const fs = require('node:fs');
const path = require('node:path');
const { execSync } = require('node:child_process');

const SERVICES_DIR = path.join(__dirname, '..', 'services');

function buildDocker() {
  const services = fs.readdirSync(SERVICES_DIR, { withFileTypes: true })
    .filter(dirent => dirent.isDirectory())
    .map(dirent => dirent.name);

  console.log('=== Docker 构建脚本 ===\n');

  for (const serviceName of services) {
    const serviceDir = path.join(SERVICES_DIR, serviceName);
    const dockerfile = path.join(serviceDir, 'Dockerfile');

    console.log(`\n## ${serviceName}`);

    if (!fs.existsSync(dockerfile)) {
      console.log('⚠️  缺失 Dockerfile，跳过构建');
      continue;
    }

    try {
      const command = `docker build -t ${serviceName}:latest ${serviceDir}`;
      console.log(`执行: ${command}`);
      execSync(command, { stdio: 'inherit' });
      console.log('✓ 构建成功');
    } catch (error) {
      console.log('✗ 构建失败');
    }
  }
}

buildDocker();
```

- [ ] **Step 2: 验证脚本存在**

Run: `Get-Content scripts\build-docker.js`
Expected: 显示脚本内容

- [ ] **Step 3: 提交脚本**

```bash
git add scripts/build-docker.js
git commit -m "feat(scripts): add docker build script"
```

---

## Task 8: 创建 Cargo 构建脚本

**Files:**
- Create: `scripts/build-cargo.js`

- [ ] **Step 1: 创建 Cargo 构建脚本**

```javascript
const fs = require('node:fs');
const path = require('node:path');
const { execSync } = require('node:child_process');

const SERVICES_DIR = path.join(__dirname, '..', 'services');

function buildCargo() {
  const services = fs.readdirSync(SERVICES_DIR, { withFileTypes: true })
    .filter(dirent => dirent.isDirectory())
    .map(dirent => dirent.name);

  console.log('=== Cargo 构建脚本 ===\n');

  for (const serviceName of services) {
    const serviceDir = path.join(SERVICES_DIR, serviceName);
    const cargoToml = path.join(serviceDir, 'Cargo.toml');

    console.log(`\n## ${serviceName}`);

    if (!fs.existsSync(cargoToml)) {
      console.log('⚠️  缺失 Cargo.toml，跳过构建');
      continue;
    }

    try {
      const command = `cargo build --release -p ${serviceName}`;
      console.log(`执行: ${command}`);
      execSync(command, { stdio: 'inherit', cwd: path.join(__dirname, '..') });
      console.log('✓ 构建成功');
    } catch (error) {
      console.log('✗ 构建失败');
    }
  }
}

buildCargo();
```

- [ ] **Step 2: 验证脚本存在**

Run: `Get-Content scripts\build-cargo.js`
Expected: 显示脚本内容

- [ ] **Step 3: 提交脚本**

```bash
git add scripts/build-cargo.js
git commit -m "feat(scripts): add cargo build script"
```

---

## Task 9: 更新 package.json 添加脚本

**Files:**
- Modify: `package.json`

- [ ] **Step 1: 读取现有 package.json**

Run: `Get-Content package.json`
Expected: 显示现有内容

- [ ] **Step 2: 添加脚本到 package.json**

在 `scripts` 部分添加：

```json
{
  "scripts": {
    "build:docker": "node scripts/build-docker.js",
    "build:cargo": "node scripts/build-cargo.js",
    "check:service-standard": "node scripts/check-service-standard.js"
  }
}
```

- [ ] **Step 3: 验证 package.json 更新**

Run: `Get-Content package.json | Select-String "build:docker"`
Expected: 显示新增脚本

- [ ] **Step 4: 提交 package.json**

```bash
git add package.json
git commit -m "feat(package.json): add service build and check scripts"
```

---

## Task 10: 更新 project_rules.md 添加触发规则

**Files:**
- Modify: `.trae/rules/project_rules.md`

- [ ] **Step 1: 读取现有 project_rules.md**

Run: `Get-Content .trae\rules\project_rules.md`
Expected: 显示现有内容

- [ ] **Step 2: 在路径 → Skill 自动匹配章节添加新规则**

在现有表格后添加：

```markdown
| `services/*/agents.md` | 自动检查服务规范 | 新建/修改 |
| `services/*/src/**/*.rs` | 检查服务规范 + coder-run-tests | 改完保存 |
```

- [ ] **Step 3: 在强约束章节添加服务规范检查**

在"修改边界"部分添加：

```markdown
### 服务规范检查

- AI 修改 `services/<name>/` 下文件时，必须先读取 `agents.md`
- 缺失必需组件时提醒用户
- 环境变量控制规范要求（SWAGGER_ENABLED、HEALTH_CHECK_REQUIRED、MIGRATIONS_REQUIRED）
```

- [ ] **Step 4: 验证 project_rules.md 更新**

Run: `Get-Content .trae\rules\project_rules.md | Select-String "agents.md"`
Expected: 显示新增规则

- [ ] **Step 5: 提交 project_rules.md**

```bash
git add .trae/rules/project_rules.md
git commit -m "chore(ai): add service standard check trigger rules"
```

---

## Task 11: 验证和测试

**Files:**
- Test: 运行检查脚本

- [ ] **Step 1: 运行服务规范检查**

Run: `node scripts/check-service-standard.js`
Expected: 显示所有服务的检查报告

- [ ] **Step 2: 检查报告分析**

分析输出，确认：
- 所有服务都有规则文件
- 缺失组件被正确标记
- 环境变量配置正确

- [ ] **Step 3: 测试 package.json 脚本**

Run: `npm run check:service-standard`
Expected: 与 Step 1 输出一致

- [ ] **Step 4: 提交验证报告**

创建验证报告文件：

```markdown
# 服务规范规则系统验证报告

## 检查结果

- 所有服务已创建规则文件 ✓
- 检查脚本运行正常 ✓
- package.json 脚本集成成功 ✓
- project_rules.md 触发规则添加成功 ✓

## 缺失组件汇总

- api-gateway: Swagger 配置、/metrics 端点
- token-service: Swagger 配置、/metrics 端点
- notification-service: Swagger 配置、/metrics 端点
- iam-service: Swagger 配置、/metrics 端点

## 下一步

- 补充 Swagger 配置
- 补充 /metrics 端点
- 更新服务 README
```

保存到 `docs/superpowers/validation/service-standard-rules-validation.md`

- [ ] **Step 5: 提交验证报告**

```bash
git add docs/superpowers/validation/service-standard-rules-validation.md
git commit -m "docs(validation): add service standard rules validation report"
```

---

## Task 12: 更新文档

**Files:**
- Modify: `AGENTS.md`

- [ ] **Step 1: 读取现有 AGENTS.md**

Run: `Get-Content AGENTS.md`
Expected: 显示现有内容

- [ ] **Step 2: 在 Skill 索引章节添加新规则**

在现有表格后添加：

```markdown
| `coder-service-standard-check` | 检查服务规范、standard | node scripts/check-service-standard.js |
```

- [ ] **Step 3: 添加服务规范说明章节**

在文档末尾添加：

```markdown
## 服务规范规则系统

### 概述

每个服务目录下 `agents.md` 定义服务开发规范。

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
```

- [ ] **Step 4: 验证 AGENTS.md 更新**

Run: `Get-Content AGENTS.md | Select-String "agents.md"`
Expected: 显示新增内容

- [ ] **Step 5: 提交 AGENTS.md**

```bash
git add AGENTS.md
git commit -m "docs(agents): add service standard rules system description"
```

---

## Self-Review

### 1. Spec Coverage

检查设计文档覆盖：
- ✓ 规则文件结构（Task 1-5）
- ✓ AI 自动检查机制（Task 10）
- ✓ 具体规范内容定义（Task 1-5 规则文件）
- ✓ package.json 脚本定义（Task 9）
- ✓ 实施步骤（Task 1-12）
- ✓ 环境变量配置（Task 1-5 规则文件）
- ✓ 与现有系统集成（Task 10）
- ✓ 验证和测试（Task 11）
- ✓ 文档和培训（Task 12）

### 2. Placeholder Scan

检查占位符：
- ✓ 无 TBD、TODO
- ✓ 无 "implement later"
- ✓ 无 "add appropriate error handling"
- ✓ 所有步骤包含完整代码
- ✓ 所有命令包含预期输出

### 3. Type Consistency

检查类型一致性：
- ✓ 规则文件格式一致
- ✓ 脚本函数名一致
- ✓ 环境变量名一致
- ✓ 文件路径一致

---

## Execution Handoff

计划完成并保存到 `docs/superpowers/plans/2026-06-13-service-standard-rules-plan.md`。

**两种执行方式：**

**1. Subagent-Driven（推荐）** - 每个任务派发新子代理，任务间审查，快速迭代

**2. Inline Execution** - 在当前会话使用 executing-plans 执行，批量执行带检查点

**选择哪种方式？**