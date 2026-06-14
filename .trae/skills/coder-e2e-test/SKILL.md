---
name: "coder-e2e-test"
description: "Runs E2E test suite for login and authentication flows. Invoke when user asks to run E2E tests, verify login flow, or check end-to-end scenarios."
---

# coder-e2e-test

## 何时用

用户说「跑E2E测试」「验证登录流程」「端到端测试」时调用本skill。

## 前置

- Docker Desktop 已启动
- 工作目录：`d:\projects\coder-micro-services`
- Node.js 18+

## 步骤

### 1. 启动基础设施

```bash
node scripts/test-orchestrator.js start
```

### 2. 重置数据库

```bash
node scripts/test-orchestrator.js reset-db
```

### 3. 运行E2E测试

```bash
node scripts/run-e2e.js
```

### 4. 检查报告

E2E测试报告输出到 `reports/run-e2e-{timestamp}/summary.md`

## 常用场景

```bash
# 验证登录功能
node scripts/run-e2e.js --test login

# 验证完整认证流程
node scripts/run-e2e.js --test auth-flow

# 验证会话管理
node scripts/run-e2e.js --test session
```

## 故障排查

- 测试失败：查看 `reports/run-e2e-*/*.log`
- 基础设施问题：`docker ps` 检查容器状态
