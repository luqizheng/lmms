# 智能体

## 概述

本文档描述 coder-micro-services 项目中的智能体/微服务。

## Skill 索引

AI 协作技能包，位于 `.trae/skills/`，按意图自动匹配：

| Skill | 触发意图 | 核心动作 |
|-------|---------|---------|
| `coder-run-tests` | 跑测试、测一下 | test-orchestrator.ps1 loop |
| `coder-fix-tests` | 修测试、fix-prompt | 渲染修复提示词 → 仅改 tests/ |
| `coder-route-validate` | 校验路由、reload | validate + /__admin/reload |
| `coder-sdk-publish` | 发 SDK、sdk:publish | mvn verify + dotnet test + push |
| `coder-spec-to-code` | 生成测试、spec-to-code | YAML 规格 → 测试代码 |
| `coder-microservice-dev` | 起服务、本地开发 | 基础设施 + 4 服务热重载 |

## 服务架构

### API 网关 (`services/api-gateway/`)

**职责：**
- 所有外部请求的入口
- 请求路由至后端服务
- 身份认证与授权
- 限流与黑名单
- 通过 Consul 进行服务发现

**核心特性：**
- JWT 令牌校验
- 基于 Redis 的黑名单管理
- 动态服务发现

### 令牌服务 (`services/token-server/`)

**职责：**
- OAuth 2.0 令牌管理
- 访问令牌生成
- 刷新令牌管理
- API 密钥管理
- JWKS 端点

**核心特性：**
- 基于 RSA 的 JWT 签名
- Redis 存储令牌
- 客户端凭证校验

### 认证服务 (`services/auth-service/`)

**职责：**
- 用户身份认证
- 密码管理
- 会话处理

**核心特性：**
- Argon2 密码哈希
- MySQL 用户存储
- 与 token-server 交换令牌

### 用户服务 (`services/user-service/`)

**职责：**
- 用户档案管理
- 基于角色的访问控制（RBAC）
- 权限管理

**核心特性：**
- 用户 CRUD 操作
- 角色与权限管理
- MySQL 数据持久化

## 通信流程

```
客户端 → API 网关 → 认证服务 → 令牌服务
                     ↓
                 用户服务
```

## 技术栈

| 服务 | 语言 | 框架 | 数据库 |
|------|------|------|--------|
| API 网关 | Rust | Axum | Redis |
| 令牌服务 | Rust | Axum | Redis |
| 认证服务 | Go | Gorilla/Mux | MySQL |
| 用户服务 | Go | Gorilla/Mux | MySQL |

## 基础设施

- **Consul**：服务发现
- **Redis**：令牌存储与缓存
- **MySQL**：持久化数据存储
- **Nginx**：反向代理

## 配置

配置文件位于 `config/` 目录：
- `api-gateway.toml` - API 网关配置
- `token-server.toml` - 令牌服务配置

## 密钥管理

加密密钥存放于 `secrets/`：
- `private.pem` - JWT 签名用 RSA 私钥
- `public.pem` - JWT 验签用 RSA 公钥
