# file-service 服务规范

## 必需组件

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ 目录
- [x] /health 端点
- [x] /metrics 端点
- [x] migrations/ 目录
- [x] Swagger 配置（SWAGGER_ENABLED 开关）

## 环境变量

配置走 `AppConfig::from_env()` 纯 env 模式（无 yaml 文件）。**`DATABASE_URL` 必填**（无 fallback，缺失或空白启动 fail-fast）。

### 通用

| 变量 | 默认值 | 说明 |
|------|--------|------|
| DATABASE_URL | **必填** | 数据库连接；缺失或空白启动 fail-fast |
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | false | 是否挂 SwaggerUi |

### 存储 driver 切换（FILE_STORAGE_BACKEND）

存储抽象走 `ObjectStore` trait，运行时按 `FILE_STORAGE_BACKEND` 选择 driver，**镜像不变，多镜像同源**即私有化部署"多客户用同一镜像"的关键。

| 变量 | 默认值 | 取值 | 说明 |
|------|--------|------|------|
| FILE_STORAGE_BACKEND | `local` | `local` / `rustfs` / `s3` | 存储 driver 别名。`rustfs` 与 `s3` 都走 S3 协议实现 `S3Storage`，仅 endpoint/path_style 默认值不同 |
| STORAGE_PATH | `./data/files` | 任意 | `local` driver 的根目录；`rustfs`/`s3` 忽略 |

### S3 协议配置（`FILE_STORAGE_BACKEND ∈ {rustfs, s3}` 必填）

| 变量 | 默认值 | 说明 |
|------|--------|------|
| S3_ENDPOINT | （rustfs 必填，s3 可选） | 例如 `http://rustfs:9000`（rustfs 默认）/ `https://oss-cn-hangzhou.aliyuncs.com`（Aliyun）/ 空（AWS 默认走 `s3.{region}.amazonaws.com`）|
| S3_BUCKET | `coder-files` | 桶名 |
| S3_REGION | `us-east-1` | 区域 |
| S3_ACCESS_KEY | **必填** | access key |
| S3_SECRET_KEY | **必填** | secret key |
| S3_PATH_STYLE | `false`（rustfs 自动 `true`）| `true` 用 path-style（`/endpoint/bucket/key`），`false` 用 vhost（`bucket.endpoint/key`）。rustfs/MinIO 必须 true；AWS/Aliyun 默认 false |
| S3_AUTO_CREATE_BUCKET | `true` | 启动时是否 attempt 创建 bucket。需要 `s3:CreateBucket` 权限；失败仅 warn，不阻断启动 |

> 私有化部署推荐组合：`FILE_STORAGE_BACKEND=rustfs` + `S3_ENDPOINT=http://rustfs:9000`（无需手设 path-style，自动开启）+ 强 access/secret。  

> AWS 公有云：`FILE_STORAGE_BACKEND=s3` + 不设 `S3_ENDPOINT`（用 AWS 标准 endpoint）+ 设 `S3_REGION=ap-east-1` 等。  

> Aliyun OSS：`FILE_STORAGE_BACKEND=s3` + `S3_ENDPOINT=https://oss-cn-hangzhou.aliyuncs.com` + `S3_REGION=oss-cn-hangzhou`（虽然 endpoint 已覆盖，仍按 oss region 习惯填）。

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /file/upload | POST | 文件上传（硬限 100MB，配置见 `src/config.rs`） |
| /file/list | GET | 文件列表查询（分页 + tenant 隔离） |
| /file/:id | GET | 下载文件内容 |
| /file/:id | DELETE | 删除文件 |
| /file/:id/info | GET | 文件元信息 |
| /file/presign | POST | 预签名 URL（`operation=get\|put`） |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

### 预签名 URL 行为

- `FILE_STORAGE_BACKEND=local`：沿用网关代理 URL（`base_url/file/{id}?expires=&token=`），不需要对象存储签名。
- `FILE_STORAGE_BACKEND=rustfs` 或 `s3`：直接返回真实的 S3 SigV4 预签名 URL（GET/PUT），客户端可直接访问对象存储 endpoint，绕过 file-service 转发流量。

## 内部端点 /file/internal/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /file/internal/migrations/state | GET | schema 状态查询（Phase 10 G3）|

- 返回 2 个 V001-V002 migration 的 marker-table 探测结果
- 鉴权：service-jwt `aud=file-service`（Phase 10 G3 service-jwt 扩面）
- 详见 `docs-site/guides/migrations-state.md`

## 存储 driver 实现矩阵

| Driver | src | 私有化部署推荐 | 公有云适配 |
|--------|-----|----------------|------------|
| LocalStorage | `storage/local.rs` | dev 单机 | — |
| S3Storage（统一 S3 协议）| `storage/s3.rs` | rustfs / MinIO | AWS S3 / Aliyun OSS / 腾讯 COS |

S3Storage 通过配置项覆盖 endpoint/path_style/region/access_key，不需新写 driver 代码 — 添加新 S3-compatible 后端只改 env。

## 豁免

（无）

## 表命名约定

所有表必须以 `file_` 前缀（共享 `superpowers` MySQL 数据库）。详见 `contracts/db-conventions.md`，CI 网关：`node scripts/check-db-naming.js`。

## AI 检查时机

修改 services/file-service/ 下文件时自动检查
