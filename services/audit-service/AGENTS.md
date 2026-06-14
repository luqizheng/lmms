# audit-service 开发规范

## 必须项

- [x] Dockerfile
- [x] 健康检查端点（/health）
- [x] 数据库迁移文件（migrations/）
- [x] config.yaml
- [x] AGENTS.md

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| DATABASE_URL | sqlite::memory: | 数据库连接 |
| SERVER_ADDR | 0.0.0.0:8080 | 服务监听地址 |
| RUST_LOG | info | 日志级别 |

## 端点域

### 操作日志
| 端点 | 方法 | 说明 |
|------|------|------|
| /audit/operation | POST | 创建操作日志 |
| /audit/operation | GET | 查询操作日志 |

### 登录日志
| 端点 | 方法 | 说明 |
|------|------|------|
| /audit/login | POST | 创建登录日志 |
| /audit/login | GET | 查询登录日志 |

### 导出
| 端点 | 方法 | 说明 |
|------|------|------|
| /audit/export/operation/csv | GET | 导出操作日志 CSV |
| /audit/export/operation/json | GET | 导出操作日志 JSON |
| /audit/export/compliance/operation/csv | GET | 按用户导出合规报告 |
| /audit/export/compliance/operation/json | GET | 按用户导出 JSON |
| /audit/export/compliance/login/csv | GET | 按用户导出登录日志 |

### 保留策略
| 端点 | 方法 | 说明 |
|------|------|------|
| /audit/retention/stats | GET | 获取保留统计 |
| /audit/retention/policy | GET | 获取保留策略 |
| /audit/retention/policy | PUT | 更新保留策略 |
| /audit/retention/cleanup | POST | 手动触发清理 |
| /audit/retention/stage/:created_at | GET | 查询数据阶段 |

## 数据保留策略

| 阶段 | 默认保留 | 说明 |
|------|----------|------|
| Hot | 90天 | 在线可查询 |
| Warm | 365天 | 近线存储 |
| Cold | 7年 | 冷归档 |

## 数据库方言

支持 MySQL 和 SQLite，通过 DATABASE_URL 自动检测。