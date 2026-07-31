# Phase 1-B：契约层（contracts）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-step. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立双端契约层，包含错误码字典、权限码字典、HTTP 约定文档及代码生成器，确保前后端一致性。

**Architecture:** YAML 契约文件作为单一真相源，Node.js 代码生成器产出 Rust 常量模块和 TypeScript 类型文件。CI 校验防止漂移。

**Tech Stack:** YAML / Node.js 18+ / Rust thiserror / TypeScript 5。

参照设计：[2026-06-13-enterprise-base-libs-design.md](file:///d:/projects/coder-micro-services/docs/superpowers/specs/2026-06-13-enterprise-base-libs-design.md) §6。

执行边界：本计划仅触及 `contracts/`、`scripts/contracts-gen.js`、`libs/digua_common-error/src/codes.rs`、`libs/digua_authz/src/perms.rs`。**不**修改 `services/`、`frontend/`。

---

## 总体文件结构

```
contracts/
├─ error-codes.yaml      # 全局错误码字典
├─ permissions.yaml      # 权限码字典
├─ events.yaml           # 领域事件名（预留）
├─ http-conventions.md   # 响应/分页/排序约定
└─ codegen/
   ├─ gen-rust.js        # → libs/digua_common-error/src/codes.rs
   ├─ gen-rust-perms.js  # → libs/digua_authz/src/perms.rs
   └─ gen-ts.js          # → frontend/src/contracts/*.ts（预留）

scripts/
└─ contracts-gen.js      # 入口脚本，调用 codegen/*.js
```

---

## Task 1：contracts 目录骨架 + error-codes.yaml

**Files:**
- Create: `contracts/error-codes.yaml`
- Create: `contracts/http-conventions.md`
- Create: `contracts/.gitkeep`

- [ ] **Step 1: 创建 contracts 目录**

```bash
mkdir -p contracts/codegen
```

- [ ] **Step 2: 写 error-codes.yaml**

`contracts/error-codes.yaml`：

```yaml
# 全局错误码字典
# 格式：<SERVICE>_<CATEGORY>_<NUMBER>
# 服务前缀：IAM / CFG / AUD / NTF / FILE / GW / SYS
# 类别：AUTH / VALID / DB / EXT / INTERNAL

version: "1.0.0"
codes:
  # === 系统级 ===
  SYS_INTERNAL_0001:
    message: "internal server error"
    kind: Internal
  SYS_DB_0001:
    message: "database error"
    kind: Internal
  SYS_DB_0002:
    message: "row not found"
    kind: NotFound
  SYS_HTTP_0001:
    message: "http client error"
    kind: External

  # === 认证 ===
  IAM_AUTH_1001:
    message: "invalid credentials"
    kind: Unauthorized
  IAM_AUTH_1002:
    message: "token expired"
    kind: Unauthorized
  IAM_AUTH_1003:
    message: "token revoked"
    kind: Unauthorized
  IAM_AUTH_1004:
    message: "invalid token"
    kind: Unauthorized
  IAM_AUTH_1005:
    message: "missing token"
    kind: Unauthorized
  IAM_AUTH_1006:
    message: "password too weak"
    kind: BadRequest
  IAM_AUTH_1007:
    message: "username already exists"
    kind: Conflict
  IAM_AUTH_1008:
    message: "email already exists"
    kind: Conflict

  # === 校验 ===
  VALID_2001:
    message: "invalid input"
    kind: BadRequest
  VALID_2002:
    message: "missing required field"
    kind: BadRequest
  VALID_2003:
    message: "invalid phone format"
    kind: BadRequest
  VALID_2004:
    message: "invalid email format"
    kind: BadRequest

  # === 权限 ===
  IAM_PERM_3001:
    message: "permission denied"
    kind: Forbidden
  IAM_PERM_3002:
    message: "role not found"
    kind: NotFound

  # === 配置 ===
  CFG_VALID_4001:
    message: "config key not found"
    kind: NotFound
  CFG_VALID_4002:
    message: "config parse error"
    kind: Internal

  # === 网关 ===
  GW_ROUTE_5001:
    message: "route not found"
    kind: NotFound
  GW_ROUTE_5002:
    message: "service unavailable"
    kind: External
  GW_RATE_5003:
    message: "rate limit exceeded"
    kind: TooManyRequests
```

- [ ] **Step 3: 写 http-conventions.md**

`contracts/http-conventions.md`：

```markdown
# HTTP 约定

## 响应格式

所有 API 响应统一 JSON 结构：

```json
{
  "code": "IAM_AUTH_1001",
  "message": "invalid credentials",
  "data": null,
  "request_id": "abc-123"
}
```

成功响应 `code` 固定为 `"OK"`，`data` 携带业务数据。

## 分页格式

```json
{
  "code": "OK",
  "message": "success",
  "data": {
    "items": [...],
    "total": 100,
    "page": 1,
    "size": 20
  },
  "request_id": "..."
}
```

## 排序约定

- 排序参数：`sort_by`（字段名） + `sort_order`（`asc` / `desc`）
- 默认：`created_at desc`

## 时间格式

- 所有时间字段：ISO8601 UTC，例 `2026-06-13T10:30:00Z`
- 字段命名：`*_at`（如 `created_at`、`updated_at`）

## ID 格式

- 所有 ID 使用 `String` 类型（雪花 i64 转 string）
- 前端不处理大整数精度问题
```

- [ ] **Step 4: 创建 .gitkeep**

`contracts/codegen/.gitkeep`：

```
# reserved for code generators
```

- [ ] **Step 5: Commit**

```bash
git add contracts
git commit -m "feat(contracts): add error-codes.yaml and http-conventions.md"
```

---

## Task 2：permissions.yaml 权限码字典

**Files:**
- Create: `contracts/permissions.yaml`
- Create: `contracts/events.yaml`

- [ ] **Step 1: 写 permissions.yaml**

`contracts/permissions.yaml`：

```yaml
# 权限码字典
# 格式：<SERVICE>_<RESOURCE>_<ACTION>
# 命名规则：大写，下划线分隔，动作后缀 CREATE / READ / UPDATE / DELETE / EXPORT / IMPORT

version: "1.0.0"
permissions:
  # === 用户管理 ===
  IAM_USER_CREATE:
    description: "创建用户"
  IAM_USER_READ:
    description: "查看用户"
  IAM_USER_UPDATE:
    description: "编辑用户"
  IAM_USER_DELETE:
    description: "删除用户"
  IAM_USER_EXPORT:
    description: "导出用户"
  IAM_USER_IMPORT:
    description: "导入用户"

  # === 角色管理 ===
  IAM_ROLE_CREATE:
    description: "创建角色"
  IAM_ROLE_READ:
    description: "查看角色"
  IAM_ROLE_UPDATE:
    description: "编辑角色"
  IAM_ROLE_DELETE:
    description: "删除角色"

  # === 权限管理 ===
  IAM_PERM_READ:
    description: "查看权限"
  IAM_PERM_ASSIGN:
    description: "分配权限"

  # === 菜单管理 ===
  IAM_MENU_CREATE:
    description: "创建菜单"
  IAM_MENU_READ:
    description: "查看菜单"
  IAM_MENU_UPDATE:
    description: "编辑菜单"
  IAM_MENU_DELETE:
    description: "删除菜单"

  # === 部门管理 ===
  IAM_ORG_CREATE:
    description: "创建部门"
  IAM_ORG_READ:
    description: "查看部门"
  IAM_ORG_UPDATE:
    description: "编辑部门"
  IAM_ORG_DELETE:
    description: "删除部门"

  # === 配置管理 ===
  CFG_CONFIG_CREATE:
    description: "创建配置项"
  CFG_CONFIG_READ:
    description: "查看配置项"
  CFG_CONFIG_UPDATE:
    description: "编辑配置项"
  CFG_CONFIG_DELETE:
    description: "删除配置项"

  # === 审计日志 ===
  AUD_LOG_READ:
    description: "查看审计日志"
  AUD_LOG_EXPORT:
    description: "导出审计日志"

  # === 通知管理 ===
  NTF_TEMPLATE_CREATE:
    description: "创建通知模板"
  NTF_TEMPLATE_READ:
    description: "查看通知模板"
  NTF_TEMPLATE_UPDATE:
    description: "编辑通知模板"
  NTF_TEMPLATE_DELETE:
    description: "删除通知模板"
  NTF_MESSAGE_READ:
    description: "查看站内信"

  # === 文件管理 ===
  FILE_UPLOAD:
    description: "上传文件"
  FILE_DOWNLOAD:
    description: "下载文件"
  FILE_DELETE:
    description: "删除文件"

  # === 系统管理 ===
  SYS_SETTING_READ:
    description: "查看系统设置"
  SYS_SETTING_UPDATE:
    description: "编辑系统设置"
```

- [ ] **Step 2: 写 events.yaml（预留）**

`contracts/events.yaml`：

```yaml
# 领域事件名（预留）
# 格式：<SERVICE>_<ENTITY>_<EVENT>
# 用于后续事件驱动架构 / 消息队列集成

version: "1.0.0"
events:
  # === 用户事件 ===
  IAM_USER_CREATED:
    description: "用户创建"
  IAM_USER_UPDATED:
    description: "用户更新"
  IAM_USER_DELETED:
    description: "用户删除"
  IAM_USER_LOGIN:
    description: "用户登录"
  IAM_USER_LOGOUT:
    description: "用户登出"

  # === 角色事件 ===
  IAM_ROLE_CREATED:
    description: "角色创建"
  IAM_ROLE_UPDATED:
    description: "角色更新"
  IAM_ROLE_DELETED:
    description: "角色删除"

  # === 配置事件 ===
  CFG_CONFIG_CHANGED:
    description: "配置变更"
```

- [ ] **Step 3: Commit**

```bash
git add contracts/permissions.yaml contracts/events.yaml
git commit -m "feat(contracts): add permissions.yaml and events.yaml"
```

---

## Task 3：Rust 错误码生成器 gen-rust.js

**Files:**
- Create: `contracts/codegen/gen-rust.js`
- Create: `libs/digua_common-error/src/codes.rs`

- [ ] **Step 1: 写 gen-rust.js**

`contracts/codegen/gen-rust.js`：

```javascript
#!/usr/bin/env node
/**
 * 从 error-codes.yaml 生成 Rust 常量模块
 * 输出：libs/digua_common-error/src/codes.rs
 */

const fs = require('fs');
const path = require('path');

// 简易 YAML 解析（仅支持本文件格式）
function parseYaml(content) {
  const lines = content.split('\n');
  const result = { codes: {} };
  let currentSection = null;
  let currentCode = null;

  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith('#') || trimmed === '') continue;

    if (trimmed.startsWith('version:')) {
      result.version = trimmed.split(':')[1].trim().replace(/"/g, '');
    } else if (trimmed === 'codes:') {
      currentSection = 'codes';
    } else if (currentSection === 'codes' && /^[A-Z_0]+:$/.test(trimmed)) {
      currentCode = trimmed.replace(':', '');
      result.codes[currentCode] = {};
    } else if (currentCode && trimmed.startsWith('message:')) {
      result.codes[currentCode].message = trimmed.split(':')[1].trim().replace(/"/g, '');
    } else if (currentCode && trimmed.startsWith('kind:')) {
      result.codes[currentCode].kind = trimmed.split(':')[1].trim();
    }
  }
  return result;
}

function generateRust(data) {
  const codes = Object.entries(data.codes);
  const lines = [
    '// Auto-generated from contracts/error-codes.yaml',
    '// DO NOT EDIT MANUALLY',
    '',
    '/// Error code constants',
    'pub mod codes {',
  ];

  for (const [code, info] of codes) {
    lines.push(`    /// ${info.message}`);
    lines.push(`    pub const ${code}: &str = "${code}";`);
  }

  lines.push('}');
  lines.push('');

  // 生成 message 映射函数
  lines.push('/// Get message for error code');
  lines.push('pub fn message(code: &str) -> &str {');
  lines.push('    match code {');
  for (const [code, info] of codes) {
    lines.push(`        "${code}" => "${info.message}",`);
  }
  lines.push('        _ => "unknown error",');
  lines.push('    }');
  lines.push('}');
  lines.push('');

  // 生成 kind 映射函数
  lines.push('use crate::ErrorKind;');
  lines.push('');
  lines.push('/// Get ErrorKind for error code');
  lines.push('pub fn kind(code: &str) -> ErrorKind {');
  lines.push('    match code {');
  for (const [code, info] of codes) {
    lines.push(`        "${code}" => ErrorKind::${info.kind},`);
  }
  lines.push('        _ => ErrorKind::Internal,');
  lines.push('    }');
  lines.push('}');

  return lines.join('\n');
}

function main() {
  const yamlPath = path.resolve(__dirname, '../error-codes.yaml');
  const outputPath = path.resolve(__dirname, '../../libs/digua_common-error/src/codes.rs');

  const yamlContent = fs.readFileSync(yamlPath, 'utf8');
  const data = parseYaml(yamlContent);
  const rustCode = generateRust(data);

  fs.writeFileSync(outputPath, rustCode);
  console.log(`Generated ${outputPath}`);
}

main();
```

- [ ] **Step 2: 运行生成器**

```bash
node contracts/codegen/gen-rust.js
```

Expected: 输出 `Generated libs/digua_common-error/src/codes.rs`

- [ ] **Step 3: 验证生成的 codes.rs**

检查 `libs/digua_common-error/src/codes.rs` 内容应包含：
- `pub mod codes { ... }` 常量模块
- `pub fn message(code: &str) -> &str` 映射函数
- `pub fn kind(code: &str) -> ErrorKind` 映射函数

- [ ] **Step 4: 更新 digua_common-error lib.rs 引用 codes**

修改 `libs/digua_common-error/src/lib.rs`，在文件末尾添加：

```rust
pub mod codes;
```

- [ ] **Step 5: 编译验证**

```bash
cargo build -p digua_common-error
```

Expected: 编译成功

- [ ] **Step 6: Commit**

```bash
git add contracts/codegen/gen-rust.js libs/digua_common-error/src/codes.rs libs/digua_common-error/src/lib.rs
git commit -m "feat(contracts): add gen-rust.js and generated codes.rs"
```

---

## Task 4：Rust 权限码生成器 gen-rust-perms.js

**Files:**
- Create: `contracts/codegen/gen-rust-perms.js`
- Create: `libs/digua_authz/src/perms.rs`

- [ ] **Step 1: 写 gen-rust-perms.js**

`contracts/codegen/gen-rust-perms.js`：

```javascript
#!/usr/bin/env node
/**
 * 从 permissions.yaml 生成 Rust 权限码常量模块
 * 输出：libs/digua_authz/src/perms.rs
 */

const fs = require('fs');
const path = require('path');

function parseYaml(content) {
  const lines = content.split('\n');
  const result = { permissions: {} };
  let currentSection = null;
  let currentPerm = null;

  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith('#') || trimmed === '') continue;

    if (trimmed.startsWith('version:')) {
      result.version = trimmed.split(':')[1].trim().replace(/"/g, '');
    } else if (trimmed === 'permissions:') {
      currentSection = 'permissions';
    } else if (currentSection === 'permissions' && /^[A-Z_]+:$/.test(trimmed)) {
      currentPerm = trimmed.replace(':', '');
      result.permissions[currentPerm] = {};
    } else if (currentPerm && trimmed.startsWith('description:')) {
      result.permissions[currentPerm].description = trimmed.split(':')[1].trim().replace(/"/g, '');
    }
  }
  return result;
}

function generateRust(data) {
  const perms = Object.entries(data.permissions);
  const lines = [
    '// Auto-generated from contracts/permissions.yaml',
    '// DO NOT EDIT MANUALLY',
    '',
    '/// Permission code constants',
    'pub mod perms {',
  ];

  for (const [perm, info] of perms) {
    lines.push(`    /// ${info.description}`);
    lines.push(`    pub const ${perm}: &str = "${perm}";`);
  }

  lines.push('}');
  lines.push('');

  // 生成检查函数
  lines.push('/// Check if permission code is valid');
  lines.push('pub fn is_valid(perm: &str) -> bool {');
  lines.push('    matches!(perm,');
  for (const [perm, _] of perms) {
    lines.push(`        "${perm}",`);
  }
  lines.push('    )');
  lines.push('}');

  return lines.join('\n');
}

function main() {
  const yamlPath = path.resolve(__dirname, '../permissions.yaml');
  const outputPath = path.resolve(__dirname, '../../libs/digua_authz/src/perms.rs');

  const yamlContent = fs.readFileSync(yamlPath, 'utf8');
  const data = parseYaml(yamlContent);
  const rustCode = generateRust(data);

  fs.writeFileSync(outputPath, rustCode);
  console.log(`Generated ${outputPath}`);
}

main();
```

- [ ] **Step 2: 运行生成器**

```bash
node contracts/codegen/gen-rust-perms.js
```

Expected: 输出 `Generated libs/digua_authz/src/perms.rs`

- [ ] **Step 3: 更新 digua_authz lib.rs 引用 perms**

修改 `libs/digua_authz/src/lib.rs`，添加：

```rust
pub mod perms;
```

- [ ] **Step 4: 编译验证**

```bash
cargo build -p digua_authz
```

Expected: 编译成功

- [ ] **Step 5: Commit**

```bash
git add contracts/codegen/gen-rust-perms.js libs/digua_authz/src/perms.rs libs/digua_authz/src/lib.rs
git commit -m "feat(contracts): add gen-rust-perms.js and generated perms.rs"
```

---

## Task 5：TypeScript 生成器 gen-ts.js（预留）

**Files:**
- Create: `contracts/codegen/gen-ts.js`

- [ ] **Step 1: 写 gen-ts.js**

`contracts/codegen/gen-ts.js`：

```javascript
#!/usr/bin/env node
/**
 * 从 error-codes.yaml 和 permissions.yaml 生成 TypeScript 类型
 * 输出：frontend/src/contracts/error-codes.ts 和 permissions.ts
 * 当前预留，待 frontend 目录建立后启用
 */

const fs = require('fs');
const path = require('path');

function parseYaml(content) {
  // 与 gen-rust.js 相同的解析逻辑
  const lines = content.split('\n');
  const result = { codes: {}, permissions: {} };
  let currentSection = null;
  let currentKey = null;

  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith('#') || trimmed === '') continue;

    if (trimmed.startsWith('version:')) {
      result.version = trimmed.split(':')[1].trim().replace(/"/g, '');
    } else if (trimmed === 'codes:') {
      currentSection = 'codes';
    } else if (trimmed === 'permissions:') {
      currentSection = 'permissions';
    } else if (currentSection && /^[A-Z_0]+:$/.test(trimmed)) {
      currentKey = trimmed.replace(':', '');
      result[currentSection][currentKey] = {};
    } else if (currentKey && trimmed.startsWith('message:')) {
      result[currentSection][currentKey].message = trimmed.split(':')[1].trim().replace(/"/g, '');
    } else if (currentKey && trimmed.startsWith('kind:')) {
      result[currentSection][currentKey].kind = trimmed.split(':')[1].trim();
    } else if (currentKey && trimmed.startsWith('description:')) {
      result[currentSection][currentKey].description = trimmed.split(':')[1].trim().replace(/"/g, '');
    }
  }
  return result;
}

function generateTsErrors(data) {
  const codes = Object.keys(data.codes);
  return [
    '// Auto-generated from contracts/error-codes.yaml',
    '// DO NOT EDIT MANUALLY',
    '',
    'export type ErrorCode =',
    ...codes.map(c => `  | '${c}'`),
    ';',
    '',
    'export const ErrorMessages: Record<ErrorCode, string> = {',
    ...Object.entries(data.codes).map(([c, i]) => `  '${c}': '${i.message}',`),
    '};',
  ].join('\n');
}

function generateTsPerms(data) {
  const perms = Object.keys(data.permissions);
  return [
    '// Auto-generated from contracts/permissions.yaml',
    '// DO NOT EDIT MANUALLY',
    '',
    'export type PermissionCode =',
    ...perms.map(p => `  | '${p}'`),
    ';',
    '',
    'export const PermissionDescriptions: Record<PermissionCode, string> = {',
    ...Object.entries(data.permissions).map(([p, i]) => `  '${p}': '${i.description}',`),
    '};',
  ].join('\n');
}

function main() {
  // 预留：待 frontend 目录建立后启用
  const frontendDir = path.resolve(__dirname, '../../frontend/src/contracts');
  if (!fs.existsSync(frontendDir)) {
    console.log('frontend directory not found, skipping TypeScript generation');
    console.log('To enable: create frontend/src/contracts/ directory');
    return;
  }

  const errorYaml = fs.readFileSync(path.resolve(__dirname, '../error-codes.yaml'), 'utf8');
  const permYaml = fs.readFileSync(path.resolve(__dirname, '../permissions.yaml'), 'utf8');

  const errorData = parseYaml(errorYaml);
  const permData = parseYaml(permYaml);

  fs.writeFileSync(
    path.join(frontendDir, 'error-codes.ts'),
    generateTsErrors(errorData)
  );
  fs.writeFileSync(
    path.join(frontendDir, 'permissions.ts'),
    generateTsPerms(permData)
  );

  console.log('Generated TypeScript contracts');
}

main();
```

- [ ] **Step 2: 运行生成器（验证预留逻辑）**

```bash
node contracts/codegen/gen-ts.js
```

Expected: 输出 `frontend directory not found, skipping TypeScript generation`

- [ ] **Step 3: Commit**

```bash
git add contracts/codegen/gen-ts.js
git commit -m "feat(contracts): add gen-ts.js (reserved for frontend)"
```

---

## Task 6：统一入口脚本 contracts-gen.js

**Files:**
- Create: `scripts/contracts-gen.js`

- [ ] **Step 1: 写 contracts-gen.js**

`scripts/contracts-gen.js`：

```javascript
#!/usr/bin/env node
/**
 * 契约代码生成入口
 * 调用 contracts/codegen/*.js 生成各语言代码
 */

const path = require('path');
const { execSync } = require('child_process');

const codegenDir = path.resolve(__dirname, '../contracts/codegen');

console.log('=== Contracts Code Generation ===');
console.log('');

// 生成 Rust 错误码
console.log('1. Generating Rust error codes...');
try {
  execSync(`node ${path.join(codegenDir, 'gen-rust.js')}`, { stdio: 'inherit' });
} catch (e) {
  console.error('Failed to generate Rust error codes');
  process.exit(1);
}

// 生成 Rust 权限码
console.log('');
console.log('2. Generating Rust permission codes...');
try {
  execSync(`node ${path.join(codegenDir, 'gen-rust-perms.js')}`, { stdio: 'inherit' });
} catch (e) {
  console.error('Failed to generate Rust permission codes');
  process.exit(1);
}

// 生成 TypeScript（预留）
console.log('');
console.log('3. Generating TypeScript contracts...');
try {
  execSync(`node ${path.join(codegenDir, 'gen-ts.js')}`, { stdio: 'inherit' });
} catch (e) {
  // TypeScript 生成失败不阻断（预留状态）
  console.log('TypeScript generation skipped (frontend not ready)');
}

console.log('');
console.log('=== Done ===');
```

- [ ] **Step 2: 运行完整生成流程**

```bash
node scripts/contracts-gen.js
```

Expected:
- 输出 `Generating Rust error codes...`
- 输出 `Generating Rust permission codes...`
- 输出 `TypeScript generation skipped`

- [ ] **Step 3: 编译验证**

```bash
cargo build -p digua_common-error -p digua_authz
```

Expected: 编译成功

- [ ] **Step 4: Commit**

```bash
git add scripts/contracts-gen.js
git commit -m "feat(scripts): add contracts-gen.js entry script"
```

---

## Task 7：CI 校验脚本（防止漂移）

**Files:**
- Create: `scripts/lib/Contracts-Validator.js`
- Modify: `scripts/test-orchestrator.js`（添加 validate-contracts 命令）

- [ ] **Step 1: 写 Contracts-Validator.js**

`scripts/lib/Contracts-Validator.js`：

```javascript
/**
 * 契约校验器
 * 检查生成的代码是否与 YAML 源文件一致
 */

const fs = require('fs');
const path = require('path');

class ContractsValidator {
  constructor() {
    this.contractsDir = path.resolve(__dirname, '../../contracts');
    this.libsDir = path.resolve(__dirname, '../../libs');
  }

  validate() {
    const results = [];

    // 校验错误码
    const errorResult = this.validateErrorCodes();
    results.push(errorResult);

    // 校验权限码
    const permResult = this.validatePermissions();
    results.push(permResult);

    return {
      passed: results.every(r => r.passed),
      results,
    };
  }

  validateErrorCodes() {
    const yamlPath = path.join(this.contractsDir, 'error-codes.yaml');
    const generatedPath = path.join(this.libsDir, 'digua_common-error/src/codes.rs');

    if (!fs.existsSync(yamlPath)) {
      return { name: 'error-codes', passed: false, error: 'YAML source not found' };
    }
    if (!fs.existsSync(generatedPath)) {
      return { name: 'error-codes', passed: false, error: 'Generated file not found' };
    }

    const yamlContent = fs.readFileSync(yamlPath, 'utf8');
    const generatedContent = fs.readFileSync(generatedPath, 'utf8');

    // 检查 YAML 中的每个 code 是否出现在生成的 Rust 文件中
    const yamlCodes = this.extractYamlCodes(yamlContent);
    const missingCodes = yamlCodes.filter(c => !generatedContent.includes(`pub const ${c}`));

    if (missingCodes.length > 0) {
      return {
        name: 'error-codes',
        passed: false,
        error: `Missing codes in generated file: ${missingCodes.join(', ')}`,
      };
    }

    return { name: 'error-codes', passed: true };
  }

  validatePermissions() {
    const yamlPath = path.join(this.contractsDir, 'permissions.yaml');
    const generatedPath = path.join(this.libsDir, 'digua_authz/src/perms.rs');

    if (!fs.existsSync(yamlPath)) {
      return { name: 'permissions', passed: false, error: 'YAML source not found' };
    }
    if (!fs.existsSync(generatedPath)) {
      return { name: 'permissions', passed: false, error: 'Generated file not found' };
    }

    const yamlContent = fs.readFileSync(yamlPath, 'utf8');
    const generatedContent = fs.readFileSync(generatedPath, 'utf8');

    const yamlPerms = this.extractYamlPermissions(yamlContent);
    const missingPerms = yamlPerms.filter(p => !generatedContent.includes(`pub const ${p}`));

    if (missingPerms.length > 0) {
      return {
        name: 'permissions',
        passed: false,
        error: `Missing permissions in generated file: ${missingPerms.join(', ')}`,
      };
    }

    return { name: 'permissions', passed: true };
  }

  extractYamlCodes(content) {
    const codes = [];
    const lines = content.split('\n');
    for (const line of lines) {
      const trimmed = line.trim();
      if (/^[A-Z_0]+:$/.test(trimmed) && !trimmed.startsWith('version')) {
        codes.push(trimmed.replace(':', ''));
      }
    }
    return codes;
  }

  extractYamlPermissions(content) {
    const perms = [];
    const lines = content.split('\n');
    for (const line of lines) {
      const trimmed = line.trim();
      if (/^[A-Z_]+:$/.test(trimmed) && !trimmed.startsWith('version')) {
        perms.push(trimmed.replace(':', ''));
      }
    }
    return perms;
  }
}

module.exports = ContractsValidator;
```

- [ ] **Step 2: 添加 validate-contracts 命令到 test-orchestrator.js**

读取 `scripts/test-orchestrator.js`，在命令列表中添加 `validate-contracts`。

- [ ] **Step 3: 运行校验**

```bash
node scripts/test-orchestrator.js validate-contracts
```

Expected: 输出 `passed: true`

- [ ] **Step 4: Commit**

```bash
git add scripts/lib/Contracts-Validator.js scripts/test-orchestrator.js
git commit -m "feat(scripts): add Contracts-Validator for drift detection"
```

---

## Task 8：contracts README 文档

**Files:**
- Create: `contracts/README.md`

- [ ] **Step 1: 写 README.md**

`contracts/README.md`：

```markdown
# contracts/ 双端契约层

契约层是前后端一致的单一真相源。

## 文件结构

| 文件 | 用途 |
|---|---|
| error-codes.yaml | 全局错误码字典 |
| permissions.yaml | 权限码字典 |
| events.yaml | 领域事件名（预留） |
| http-conventions.md | HTTP 响应/分页/排序约定 |

## 代码生成

```bash
node scripts/contracts-gen.js
```

生成目标：
- `libs/digua_common-error/src/codes.rs` — Rust 错误码常量
- `libs/digua_authz/src/perms.rs` — Rust 权限码常量
- `frontend/src/contracts/*.ts` — TypeScript 类型（预留）

## 命名规则

### 错误码

格式：`<SERVICE>_<CATEGORY>_<NUMBER>`

- 服务前缀：`IAM` / `CFG` / `AUD` / `NTF` / `FILE` / `GW` / `SYS`
- 类别：`AUTH` / `VALID` / `DB` / `EXT` / `INTERNAL`

### 权限码

格式：`<SERVICE>_<RESOURCE>_<ACTION>`

- 动作后缀：`CREATE` / `READ` / `UPDATE` / `DELETE` / `EXPORT` / `IMPORT`

## CI 校验

```bash
node scripts/test-orchestrator.js validate-contracts
```

检查生成的代码是否与 YAML 源文件一致，防止漂移。

## 编辑流程

1. 编辑 YAML 文件
2. 运行 `node scripts/contracts-gen.js`
3. 运行 `cargo build -p digua_common-error -p digua_authz`
4. Commit YAML + 生成的 Rust 文件
```

- [ ] **Step 2: Commit**

```bash
git add contracts/README.md
git commit -m "docs(contracts): add README with naming rules and workflow"
```

---

## 自查（plan self-review）

1. Spec 覆盖：
   - §6.1 目录结构 → Task 1-8 全覆盖
   - §6.2 命名规则 → README.md + YAML 文件
   - 代码生成 → Task 3-6
   - CI 校验 → Task 7
2. Placeholder scan：无 TBD / TODO / "implement later"
3. Type consistency：
   - `ErrorCode` → `codes.rs` 常量
   - `PermissionCode` → `perms.rs` 常量
   - YAML → Rust 映射函数一致

---

## 执行选择

**Plan complete and saved.** 两种执行方式：

1. **Subagent-Driven（推荐）**——每个 Task 派出新 subagent，逐 Task review。
2. **Inline Execution**——本会话 executing-plans，分段执行加 checkpoint。

请告知选择哪种。