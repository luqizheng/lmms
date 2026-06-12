---
name: coder-spec-to-code
description: 把 tests/specs/*.yaml 测试规格转成 AI 提示词，生成 Go/Rust 测试代码。用于「生成测试」「spec-to-code」「写测试用例」等意图。
---

# coder-spec-to-code

## 何时用

- 新增 `tests/specs/<service>/*.yaml` 后
- 用户说「生成测试」「spec-to-code」「按规格写测试」时

## 步骤

### 1. 校验规格

```bash
cd d:\projects\coder-micro-services
node scripts/test-orchestrator.js validate-specs
```

必填字段：`name` / `service` / `type` / `description` / `steps`

### 2. 生成提示词

```bash
node scripts/test-orchestrator.js generate-prompt
```

输出：`reports\spec-to-code-prompt.md`

### 3. 贴给 AI

模板里含所有 `tests/specs/**/*.yaml` 内容，AI 输出代码块。

代码落点：
- Go: `services/<service>/<name>_test.go`（与被测代码同包）
- Rust: `services/<service>/tests/<name>.rs`

### 4. 跑生成的测试

```bash
node scripts/test-orchestrator.js reset-db
node scripts/test-orchestrator.js test-integration --service <service>
```

## YAML 字段

```yaml
name: login_with_valid_credentials
service: auth-service
type: integration
description: 有效凭证登录返回 access_token
tags: [smoke, auth]
preconditions:
  - 数据库存在测试用户 test_user / Passw0rd!
steps:
  - method: POST
    path: /auth/login
    body:
      username: test_user
      password: Passw0rd!
expected:
  status: 200
  body_contains: access_token
data_cleanup:
  - DELETE FROM users WHERE username='test_user'
```

`name` 必须匹配 `^[a-z][a-z0-9_]*$`。
`service` ∈ {auth-service, user-service, api-gateway, token-server}。
`type` ∈ {unit, integration}。

## 模板位置

`scripts\prompts\spec-to-code.template.md`
