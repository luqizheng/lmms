# audit-service

审计日志服务，提供操作日志、登录日志、合规导出能力。

## 功能

- 操作日志：业务操作审计，支持按用户/动作/资源检索
- 登录日志：认证审计，记录成功/失败登录
- CSV 导出：支持合规导出

## 快速开始

```bash
# SQLite 测试
DATABASE_URL=sqlite::memory: cargo run

# MySQL
DATABASE_URL=mysql://user:pwd@localhost/audit cargo run
```

## API

```bash
# 创建操作日志
curl -X POST http://localhost:8080/audit/operation \
  -H "Content-Type: application/json" \
  -d '{"user_id":"u1","action":"user.update","resource":"user/123","request_id":"req-001"}'

# 查询操作日志
curl http://localhost:8080/audit/operation

# 创建登录日志
curl -X POST http://localhost:8080/audit/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","success":true,"ip_address":"192.168.1.1","request_id":"req-002"}'

# 导出 CSV
curl http://localhost:8080/audit/export/operation/csv
```

## 环境变量

- DATABASE_URL: 数据库连接
- SERVER_ADDR: 监听地址（默认 0.0.0.0:8080）
- RUST_LOG: 日志级别（默认 info）
