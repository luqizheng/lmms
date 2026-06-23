---
name: "coder-service-audit"
description: "Audits services against AGENTS.md standards. Invoke when user asks to check service health, verify service standards, or audit service compliance."
---

# coder-service-audit

## 何时用

用户说「检查服务规范」「审计服务」「服务健康检查」「是否符合标准」时调用本skill。

## 前置

- 运行环境：WSL / macOS / Linux（Windows 必须在 WSL 内执行）
- 工作目录：项目根目录
- Node.js 18+

## 步骤

### 1. 运行服务规范检查

```bash
node scripts/check-service-standard.js
```

### 2. 检查单个服务

```bash
node scripts/check-service-standard.js --service <service-name>
```

### 3. 验证规范项

按AGENTS.md标准逐项核对：

| 检查项 | 说明 |
|--------|------|
| Dockerfile | 服务必须有Dockerfile |
| Swagger | 环境变量SWAGGER_ENABLED控制 |
| Health Check | 环境变量HEALTH_CHECK_REQUIRED |
| Migrations | 环境变量MIGRATIONS_REQUIRED |
| README.md | 服务文档 |
| tests/ | 测试目录 |

### 4. 生成审计报告

```bash
node scripts/check-service-standard.js --service <name> --report
```

## 服务名清单

- `token-service`
- `api-gateway`
- `notification-service`
- `audit-service`
- `config-service`
- `file-service`
- `iam-service`

## 常用场景

```bash
# 检查全部服务
node scripts/check-service-standard.js --all

# 检查指定服务
node scripts/check-service-standard.js --service iam-service

# 输出详细报告
node scripts/check-service-standard.js --service api-gateway --verbose
```

## 故障排查

- 缺少文件：根据AGENTS.md创建缺失组件
- 规范不符：按环境变量要求补充配置
