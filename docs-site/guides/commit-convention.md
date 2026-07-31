# Commit Message 规范

> **状态**：草案 · 待规范评审
> **关联**：onboarding/08-常见任务速查.md §8.7 TODO 标记
> **适用范围**：coder-micro-services 所有 git 提交

---

## 0. 一句话

> 使用 Conventional Commits 格式，一个 commit 做一件事，用 imperative mood 描述。

---

## 1. 格式

### 1.1 完整格式

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

### 1.2 简短格式

```
<type>: <description>
```

用于简单改动，不需要 scope。

### 1.3 示例

```
feat(iam): add user role assignment endpoint

Implement PUT /v1/iam/users/{id}/roles to assign roles to users.
This replaces the previous multi-step process with a single API call.

Closes #123
```

---

## 2. Type 类型

### 2.1 类型列表

| Type | 说明 | 示例 |
|------|------|------|
| `feat` | 新功能 | 新增接口、新增页面 |
| `fix` | 错误修复 | 修复登录 bug、修复内存泄漏 |
| `docs` | 文档更新 | 补充 README、修复文档错误 |
| `style` | 格式调整（不改代码逻辑） | 格式化代码、整理 import |
| `refactor` | 重构（不改功能） | 提取函数、简化逻辑 |
| `perf` | 性能优化 | 提升查询速度、优化算法 |
| `test` | 测试相关 | 新增单元测试、修复测试 |
| `chore` | 维护任务 | 升级依赖、配置文件更新 |
| `ci` | CI/CD 改动 | 修改 GitLab CI、添加 action |
| `revert` | 回滚提交 | revert: feat(iam): add login |
| `merge` | 合并分支 | Merge branch 'feature/abc' |

### 2.2 使用原则

- **feat** 和 **fix** 最常用，几乎每个 commit 都会用到
- **refactor** 用于较大的代码结构变动，不是 bug 修复也不是新功能
- **chore** 用于依赖更新、配置文件修改等杂项
- 不要用 **feat** 来做 **fix** 的事，也不要反过来

---

## 3. Scope 范围

### 3.1 常用 Scope

| Scope | 含义 |
|-------|------|
| `iam` | iam-service |
| `token` | token-service |
| `notification` | notification-service |
| `audit` | audit-service |
| `config` | config-service |
| `file` | file-service |
| `gateway` | api-gateway |
| `prue` | prue_backend |
| `e2e` | 端到端测试 |
| `db` | 数据库迁移 |
| `infra` | 基础设施 |
| `lib` | 公共库 |
| `docs` | 文档 |

### 3.2 多个 Scope

如果一个 commit 涉及多个 scope，可以用逗号分隔：

```
feat(iam,token): add service-jwt support
```

但这种情况较少，更常见的做法是拆成多个 commit。

---

## 4. Description 描述

### 4.1 规则

- 使用 **imperative mood**（命令式）「Add feature」而不是「Added feature」或「Adds feature」
- 首字母小写
- 不加句号
- 不超过 72 字符
- 简洁明了，不废话

### 4.2 好/坏示例

```
✅ feat(iam): add user role assignment endpoint
❌ feat: added new feature for assigning roles to users

✅ fix(gateway): handle null trace_id in log
❌ Fixed bug where sometimes there was a null value

✅ docs: add troubleshooting guide
❌ Updated documentation

✅ refactor(token): extract JWT signing logic
❌ Refactored some stuff
```

### 4.3 Imperative Mood 参考

| 正确 | 错误 |
|------|------|
| Add user endpoint | Added user endpoint |
| Fix login bug | Fixed login bug |
| Update README | Updated README |
| Remove deprecated code | Removed deprecated code |
| Implement config watch | Implemented config watch |

---

## 5. Body 正文

### 5.1 何时使用

- commit 比较复杂，需要解释「做什么」和「为什么」
- 涉及业务逻辑变更
- 需要记录迁移步骤、注意事项

### 5.2 格式

- 与 description 之间空一行
- 每行不超过 72 字符
- 使用完整句子，描述清楚

### 5.3 示例

```
feat(iam): add department tree API

Add GET /v1/iam/org-units/tree endpoint that returns
the full organization tree in a single response.

This is more efficient for the frontend than making
multiple requests to the flat list endpoint.
```

---

## 6. Footer 结尾

### 6.1 关联 Issue/PR

```
Closes #123
Fixes #456
Related to #789
```

### 6.2 Breaking Changes

```
BREAKING CHANGE: remove deprecated /v1/auth/login endpoint

The old endpoint has been replaced by /v1/iam/auth/login.
Migration guide: https://docs.example.com/migration
```

---

## 7. Commit 类型与分支策略

### 7.1 分支命名

| 分支类型 | 命名格式 | 示例 |
|----------|----------|------|
| 功能分支 | `feature/<scope>-<short-description>` | `feature/iam-dept-tree` |
| 修复分支 | `fix/<scope>-<short-description>` | `fix/gateway-rate-limit` |
| 验证分支 | `validation/<topic>` | `validation/oauth-integration` |
| 发布分支 | `release/v<version>` | `release/v1.2.0` |
| 热修复 | `hotfix/<description>` | `hotfix/critical-auth-bug` |

### 7.2 Commit 顺序

一个功能/任务的完整开发流程：

```
feat(iam): add department tree model
feat(iam): add department tree migration
feat(iam): add department tree API
feat(iam): add department tree unit tests
fix(iam): fix tree circular reference bug
docs(iam): add department tree API docs
```

---

## 8. Git Hygiene

### 8.1 不要做的事

```
❌ 不要 force-push 到 main/develop
❌ 不要 amend 已推送的 commit
❌ 不要在 main 上直接 commit
❌ 不要一个 commit 做太多事（超过 500 行改动）
```

### 8.2 Amend 的正确姿势

只在**本地**未推送的 commit 上 amend：

```bash
# 1. 刚 commit，发现有个 typo
git commit -m "feat(iam): add user API"
# 发现 typo

# 2. 修改 + amend（仅限本地未推送）
git add .
git commit --amend
# 修改 message 或追加改动

# 3. 如果已经推送，不要 amend
# 而是新开一个 fix commit
git commit -m "fix(iam): correct typo in user API"
```

### 8.3 Squash 多个小 commit

```bash
# 把最近 3 个 commit 合成一个
git rebase -i HEAD~3
# 在 editor 里把第二、三个 pick 改成 squash
```

---

## 9. CI 集成

### 9.1 commitlint 配置

项目可配置 `commitlint` 做 commit message 检查：

```json
// commitlint.config.js
module.exports = {
  extends: ['@commitlint/config-conventional'],
  rules: {
    'type-enum': [
      2,
      'always',
      ['feat', 'fix', 'docs', 'style', 'refactor', 'perf', 'test', 'chore', 'ci', 'revert'],
    ],
    'scope-enum': [
      2,
      'always',
      ['iam', 'token', 'notification', 'audit', 'config', 'file', 'gateway', 'prue', 'pm', 'e2e', 'db', 'infra', 'lib', 'docs'],
    ],
  },
};
```

### 9.2 pre-commit hook

```bash
# 安装 pre-commit
npm install -g @commitlint/cli
npx husky install
npx husky add .husky/commit-msg 'npx commitlint --edit $1'
```

---

## 10. 示例集

### 10.1 功能开发

```
feat(iam): add user department change event

Publish iam.user.dept.changed event when user's department
is modified. Event payload includes user_id, old_dept_id,
new_dept_id, and changed_at timestamp.

Part of Phase 4.4-fu1
```

### 10.2 Bug 修复

```
fix(gateway): prevent panic on malformed traceparent header

When traceparent header has invalid format, tracing middleware
now returns 400 instead of panicking. Added input validation
with regex pattern matching W3C Trace Context spec.

Fixes: #234
```

### 10.3 数据库迁移

```
feat(db): add tenant_id column to iam_users

Phase 8.2 migration. Add nullable tenant_id column to iam_users
table. Application-level enforcement will be added in follow-up.

WARNING: Do not run this migration concurrently with writes to
iam_users table. Use downtime window or pt-online-schema-change.
```

### 10.4 文档更新

```
docs: add troubleshooting section for login failures

Document digua_common causes of login failures (wrong password,
account locked, token expired) with resolution steps.
Cross-reference with jaeger tracing guide for trace-based debugging.
```

### 10.5 重构

```
refactor(token): extract JWT signing into separate module

Move JWT signing logic from main.rs into src/jwt_sign.rs.
Prepare for kid-based key rotation support.

No functional change.
```

### 10.6 依赖更新

```
chore(deps): upgrade tokio to 1.35

Upgrade tokio from 1.28 to 1.35 for performance improvements.
Update Cargo.lock and verify all tests pass.
```

---

## 11. 关联

- `.trae/rules/project_rules.md` — 项目规则
- `onboarding/08-常见任务速查.md` — 常见任务入口
- Conventional Commits 规范：https://www.conventionalcommits.org/

---

> **最后更新**：2026-07-13
