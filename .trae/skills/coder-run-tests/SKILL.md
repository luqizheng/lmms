---
name: coder-run-tests
description: 调用 test-orchestrator.js 跑 coder-micro-services 全量或单服务测试。用于「跑测试」「测一下」「执行测试」等意图。
---

# coder-run-tests

## 何时用

用户说「跑测试」「测一下」「test-all」「test-unit」「test-integration」时调用本 skill。

## 前置

- Docker Desktop 已启动
- 工作目录：`d:\projects\coder-micro-services`
- Node.js 18+

## 步骤

### 1. 拉起基础设施

```bash
cd d:\projects\coder-micro-services
node scripts/test-orchestrator.js start
```

### 2. 重置数据库

```bash
node scripts/test-orchestrator.js reset-db
```

### 3. 运行测试

```bash
# 全量
node scripts/test-orchestrator.js test-all

# 单服务
node scripts/test-orchestrator.js test-all --service api-gateway

# 单类型
node scripts/test-orchestrator.js test-unit
node scripts/test-orchestrator.js test-integration
```

### 4. 生成报告

```bash
node scripts/test-orchestrator.js report
```

报告路径：`reports\run-{ts}\summary.md`

## 快捷：一键闭环

```bash
node scripts/test-orchestrator.js loop
```

= start + reset-db + test-all + report

## 停止

```bash
node scripts/test-orchestrator.js stop
```

## 故障

- 容器起不来：`docker ps -a` 看状态
- 端口冲突：先 `stop` 再 `start`
- 编译错误：看 `reports\run-{ts}\*.log`
