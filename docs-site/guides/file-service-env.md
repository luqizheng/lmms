# file-service 环境变量

配置加载优先级：**代码默认值 ← 环境变量 ← docker-compose environment**

## 非配置字段（main.rs / config.rs 直接读取）

| 中文 | env | docker-compose 值 | 代码默认值 |
|------|-----|-------------------|-----------|
| 数据库连接 URL | `DATABASE_URL` | `${DATABASE_URL}` | `mysql://root:root@127.0.0.1:3306/superpowers?charset=utf8mb4` |
| 监听地址 | `SERVER_ADDR` | `0.0.0.0:8082` | `0.0.0.0:8082` |
| 日志级别 | `RUST_LOG` | `info` | `info` |
| 文件存储路径 | `STORAGE_PATH` | `/var/lib/file-service/storage` | `./data/files` |
| 最大文件大小（字节） | — | — | `104857600`（100MB，硬编码） |
| JWKS 地址 | `JWKS_URL` | `${JWKS_URL}` | `http://token-service:8082/.well-known/jwks.json` |
| 是否启用 Swagger UI | `SWAGGER_ENABLED` | `false`（注释默认值） | `false` |
| OpenTelemetry 端点 | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://otel-collector:4317` | — |
| 服务名（通用标识） | `SERVICE_NAME` | `file-service` | `file-service` |
| 服务实例 ID | `SERVICE_ID` | `file-service` | `file-service` |
| 服务端口（注册探测） | `SERVICE_PORT` | `8082` | `8080` |
| 健康检查路径 | `SERVICE_HEALTH_PATH` | `/health` | `/health` |

> **注意**：`SERVICE_BOOTSTRAP_TOKEN`、`TENANT_ID`、`TOKEN_SERVICE_URL` 在 file-service 代码中**未使用**，已从 docker-compose 移除。