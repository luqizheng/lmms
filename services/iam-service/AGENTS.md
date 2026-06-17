# iam-service 服务规范

## 必需组件

- [x] Dockerfile
- [x] Swagger 配置（可配置启用/禁用）
- [x] 健康检查端点（/health、/metrics）
- [x] 数据库迁移文件（migrations/）
- [x] README.md
- [x] tests/ 目录

## 配置选项

- swagger_enabled: 环境变量 SWAGGER_ENABLED 或默认 true
- health_check_required: 环境变量 HEALTH_CHECK_REQUIRED 或默认 true
- migrations_required: 环境变量 MIGRATIONS_REQUIRED 或默认 true

## 端点域

### 认证 /iam/auth/*
| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/auth/login | POST | 用户登录 |
| /iam/auth/refresh | POST | 刷新 Token |
| /iam/auth/logout | POST | 用户登出 |
| /iam/auth/password | POST | 修改密码 |
| /iam/auth/sessions | GET | 获取会话列表 |
| /iam/auth/sessions/:id | DELETE | 撤销会话 |

### 用户 /iam/users/*
| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/users | GET | 查询用户列表 |
| /iam/users | POST | 创建用户 |
| /iam/users/:id | GET | 获取用户详情 |
| /iam/users/:id | PUT | 更新用户 |
| /iam/users/:id | DELETE | 删除用户 |

### 角色 /iam/roles/*
| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/roles | GET | 查询角色列表 |
| /iam/roles | POST | 创建角色 |
| /iam/roles/:id | GET | 获取角色详情 |
| /iam/roles/:id | PUT | 更新角色 |
| /iam/roles/:id | DELETE | 删除角色 |
| /iam/roles/:id/permissions | PUT | 绑定权限 |

### 权限 /iam/permissions/*
| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/permissions | GET | 查询权限列表 |
| /iam/permissions | POST | 创建权限 |
| /iam/permissions/:id | GET | 获取权限详情 |
| /iam/permissions/:id | PUT | 更新权限 |
| /iam/permissions/:id | DELETE | 删除权限 |

### 组织单元 /iam/org-units/*
| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/org-units | GET | 查询组织列表（树形） |
| /iam/org-units | POST | 创建组织 |
| /iam/org-units/:id | GET | 获取组织详情 |
| /iam/org-units/:id | PUT | 更新组织 |
| /iam/org-units/:id | DELETE | 删除组织 |

## 数据库方言

支持 MySQL 和 SQLite。

## 系统端点
| 端点 | 方法 | 说明 |
|------|------|------|
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |

## 状态说明

本服务整合了 token-service、credential-login-service、user-service 的功能。
