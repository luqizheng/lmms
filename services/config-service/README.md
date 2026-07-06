# config-service

配置中心服务，提供多环境配置管理、版本控制、灰度发布能力。

## 功能

- 命名空间管理：app + env + profile
- 配置项 CRUD：支持多种类型（string/int/bool/json/yaml）
- 版本控制：变更历史追踪
- 长轮询/SSE 推送：配置热更新
- 导入导出：JSON 格式

## 快速开始

```bash
# MySQL（已移除 SQLite 支持）
DATABASE_URL=mysql://user:pwd@localhost/superpowers cargo run
```

## API

```bash
# 创建命名空间
curl -X POST http://localhost:8081/config/namespace \
  -H "Content-Type: application/json" \
  -d '{"app":"myapp","env":"prod","description":"Production config"}'

# 创建配置项
curl -X POST http://localhost:8081/config/item \
  -H "Content-Type: application/json" \
  -d '{"namespace_id":"ns-id","key":"database.host","value":"localhost","value_type":"string"}'

# 查询配置项
curl http://localhost:8081/config/item?namespace_id=ns-id

# 导出配置
curl http://localhost:8081/config/export/ns-id

# 长轮询监听
curl "http://localhost:8081/config/watch?namespace_id=ns-id&timeout=30"
```

## 环境变量

- DATABASE_URL: 数据库连接
- SERVER_ADDR: 监听地址（默认 0.0.0.0:8081）
- RUST_LOG: 日志级别（默认 info）

## 数据模型

### Namespace
```json
{
  "id": "uuid",
  "app": "string",
  "env": "string",
  "profile": "string (optional)",
  "description": "string (optional)"
}
```

### ConfigItem
```json
{
  "id": "uuid",
  "namespace_id": "string",
  "key": "string",
  "value": "string",
  "value_type": "string (string/int/bool/json/yaml)",
  "is_secret": "boolean",
  "description": "string (optional)"
}
```
