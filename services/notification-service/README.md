# notification-service

通知服务，支持 WebSocket 实时推送和消息调度。

## 功能模块

- **REST API** (`/api/notifications/*`) - 通知管理
- **WebSocket** (`/api/notifications/ws`) - 实时推送
- **Scheduler** - 消息定时调度

## 技术栈

- Rust 1.78+
- Axum 0.8 (Web 框架)
- SQLx (数据库)
- Redis (缓存/队列)
- WebSocket (实时推送)

## 配置

配置文件：`config/config.yaml`

```yaml
server:
  http_addr: 0.0.0.0:8083

mysql:
  url: mysql://root:password@localhost:3306/notification_db
  max_connections: 20

redis:
  url: redis://localhost:6379/0

scheduler:
  tick_secs: 60

log:
  level: info

consul:
  addr: http://localhost:8500
  service_name: notification-service
  service_id_file: /tmp/notification-service.id
```

## 数据库迁移

迁移文件：`migrations/2026_06_10_init.sql`

## 构建

```bash
cargo build --manifest-path services/notification-service/Cargo.toml
```

## 运行

```bash
cargo run --manifest-path services/notification-service/Cargo.toml
```

## Docker

```bash
docker build -t notification-service:latest services/notification-service/
docker run -p 8083:8083 notification-service:latest
```

## 端点

- `GET /health` - 健康检查
- `GET /metrics` - Prometheus 指标端点
- `GET /swagger-ui` - Swagger UI 文档
- `GET /api-docs/openapi.json` - OpenAPI 规范
- `POST /api/notifications` - 发送通知
- `GET /api/notifications` - 获取通知列表
- `GET /api/notifications/{id}` - 获取通知详情
- `POST /api/notifications/{id}/read` - 标记已读
- `GET /api/notifications/ws` - WebSocket 连接

## 测试

```bash
cargo test --manifest-path services/notification-service/Cargo.toml
```