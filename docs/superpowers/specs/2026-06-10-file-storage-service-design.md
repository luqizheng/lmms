# 文件存储服务（file-storage-service）设计

> 设计日期: 2026-06-10
> 状态: Draft → 待审核
> 关联: `2026-06-10-microservice-jwt-verification-design.md`、`2026-06-10-go-to-rust-migration-design.md`

---

## 1. 概述

为 coder-micro-services 提供统一文件存储能力，核心目标：

1. **去重**：相同内容文件只存一份，多方通过引用表关联
2. **多租户 + 多方引用**：记录文件归属的租户与引用方（user / service）
3. **备份可选**：通过 `backup.enabled` 配置开关启用异地增量备份

**关键决策**
- 服务归属：新建 `services/file-storage-service`（Rust + Axum）
- 存储后端：单节点 **RustFS**（S3 兼容）
- 元数据：MySQL（`files`、`file_refs` 两表）
- 去重粒度：**整文件 SHA-256**
- 备份：`restic` 异地 S3 桶，默认关闭
- 鉴权：复用 `token-server` JWT（api-gateway 注入 `X-User-*`）

---

## 2. 总体架构

```
   浏览器 / 服务端
     │  HTTPS REST + Bearer JWT
     ▼
  api-gateway (Rust)              ← 现有入口
     │  验签 + 注入 X-User-Id / X-User-Roles / X-Tenant-Id
     ▼
  file-storage-service (Rust)     ← 新增
     │  ┌──────────┐ ┌──────────┐ ┌──────────────┐
     │  │ REST API │ │ Dedupe   │ │ Backup Cron  │
     │  └──────────┘ └──────────┘ └──────────────┘
     │       │             │              │
     │       └──────┬──────┴──────────────┘
     │              │
     │         ┌────▼────┐        ┌─────────┐
     │         │  MySQL  │  S3    │ RustFS  │
     │         │ 元数据  │◄──────►│ blob    │
     │         └─────────┘        └─────────┘
     │
     │  （可选）restic backup  → 异地 S3
```

端口 `:8084`，Consul 注册名 `file-storage-service`。

---

## 3. 数据模型

复用 `mysql:8.0`，DB 名 `file_storage`。

```sql
-- blob 维度（一个 hash 物理一份）
files (
  hash        CHAR(64)      PRIMARY KEY,        -- SHA-256 hex
  size        BIGINT UNSIGNED NOT NULL,
  mime        VARCHAR(128)  NOT NULL,
  s3_key      VARCHAR(255)  NOT NULL,           -- RustFS 对象 key
  ref_count   INT UNSIGNED  NOT NULL DEFAULT 0, -- 冗余计数
  orphan      TINYINT(1)    NOT NULL DEFAULT 0, -- 上传成功但 DB 失败标记
  created_at  DATETIME      NOT NULL
)

-- 引用维度（多租户 + 多方）
file_refs (
  id          BIGINT UNSIGNED PK AUTO_INCREMENT,
  tenant_id   VARCHAR(64)   NOT NULL,
  owner_type  ENUM('user','service') NOT NULL,
  owner_id    VARCHAR(64)   NOT NULL,           -- userId / service 名
  hash        CHAR(64)      NOT NULL,
  file_name   VARCHAR(255)  NOT NULL,           -- 引用方视角的展示名
  created_at  DATETIME      NOT NULL,
  UNIQUE KEY uk_tenant_owner_hash (tenant_id, owner_type, owner_id, hash),
  KEY idx_hash (hash),
  CONSTRAINT fk_refs_hash FOREIGN KEY (hash) REFERENCES files(hash) ON DELETE CASCADE
)
```

**租户隔离**：所有查询强制带 `tenant_id` 过滤；删除 / 读取都校验归属。

---

## 4. 接口

路径前缀 `/api/files`，全部经 api-gateway 转发；网关已注入 `X-User-*`。

| Method | Path | 说明 |
|---|---|---|
| POST | `/` | 上传（multipart 或 raw body），header 必带 `X-File-Name` |
| GET | `/{ref_id}` | 302 重定向至 RustFS 预签名 URL（5min 过期） |
| DELETE | `/{ref_id}` | 删引用，ref_count=0 时清理 S3 对象 |
| GET | `/{ref_id}/meta` | 返回 `{file_id, ref_id, file_name, size, mime, hash, created_at}` |

**请求/响应**

POST 响应：
```json
{
  "file_id": "e3b0c4...",
  "ref_id": 1024,
  "hash": "e3b0c4...",
  "size": 204800,
  "mime": "application/pdf",
  "deduped": true
}
```

`deduped=true` 表示命中已有 blob，未重复上传。

**鉴权**
- Bearer JWT 解析 `sub`（user_id）、`tenant_id`、`roles`
- `tenant_id` 来自 JWT claim（网关已注入到 `X-Tenant-Id`）
- `owner_type=service` 需 JWT 含 `service` scope，调用方传 `X-Owner-Id`（service 名）

---

## 5. 关键流程

### 5.1 上传
1. 解析 header：`X-Tenant-Id`、`X-Owner-Type`（默认 `user`）、`X-Owner-Id`（默认 JWT sub）、`X-File-Name`
2. 流式计算 SHA-256（边读边算，O(1) 内存）
3. 查 `files` 行：
   - 命中 + 非孤儿 → 进入 5.2 复用路径
   - 未命中 → 进入 5.3 新增路径
4. 写 `file_refs`（`INSERT IGNORE` 兜底并发）
5. 返回响应

### 5.2 复用（命中）
- `file_refs` 已存在（同一 tenant + owner + hash）→ 直接返回 ref_id，`ref_count` 不变
- `file_refs` 不存在但 files 存在 → INSERT ref，`ref_count + 1`（事务）

### 5.3 新增
1. 事务开启
2. 生成 `s3_key = {tenant_id}/{hash[:2]}/{hash}`
3. `aws-sdk-s3` PutObject（流式，content-sha256 校验）
4. INSERT `files`（`orphan=0`，`ref_count=1`）
5. INSERT `file_refs`
6. 事务提交
7. 失败补偿：S3 DeleteObject（best-effort），日志告警

### 5.4 删除引用
1. 校验 `file_refs.tenant_id` 匹配 JWT
2. 事务：
   - DELETE `file_refs`（受影响 1 行才继续）
   - SELECT COUNT `file_refs` WHERE `hash=?`（锁定读）
   - 若计数 = 0：
     - DELETE `files`（`ON DELETE CASCADE` 兜底）
     - 事务外 S3 DeleteObject（best-effort）

### 5.5 孤儿清理（worker）
- tokio interval 5min
- 扫 `files WHERE orphan=1 AND created_at < NOW() - INTERVAL 10 MINUTE`
- S3 DeleteObject + DELETE `files`（级联删 refs）

### 5.6 备份（可选，cron）
- 启用时每天 02:00 执行 `restic backup /var/rustfs-data`
- 备份数据 + MySQL dump（`mysqldump file_storage` 单独打一份）

---

## 6. 错误处理

| 场景 | 行为 |
|---|---|
| 流式 hash 与 S3 content-md5 不一致 | 上传中断，标记 orphan，worker 清理 |
| S3 上传成功 + DB 写入失败 | 标记 `orphan=1`，worker 清理 |
| 重复引用并发 | `INSERT IGNORE` 兜底，affected=0 时复用 |
| tenant 越权访问 ref_id | 404（不暴露存在性） |
| ref_count 漂移 | 定期对账任务：COUNT(file_refs) 覆写 files.ref_count |
| S3 不可用 | 503 + 重试（指数退避 3 次） |
| RustFS 磁盘满 | 422 + 告警 |
| 备份 enabled 但 repo/密码缺失 | 启动失败，配置校验 |
| 预签名 URL 过期 | 客户端重新 GET 拿新 URL |

---

## 7. 测试

| 层 | 覆盖 |
|---|---|
| 单元 | SHA-256 流式、ref_count 增减、INSERT IGNORE 兜底、tenant 隔离 |
| 集成（in-process） | 上传-秒传-删除闭环、孤儿清理、对账修复 ref_count |
| 集成（HTTP） | REST 全路径、header 缺失、越权 404 |
| E2E | gateway → file-storage：上传 → 引用 → 删除引用 → S3 对象消失 |
| 备份 | mock restic 进程：成功路径、配置缺失启动失败、enabled=false 不执行 |
| 规格 | `tests/specs/file-storage-service/files.yaml`（YAML 行为规格） |

测试运行复用 `scripts/test-orchestrator.ps1`，新增 `-Service file-storage-service`。

---

## 8. 部署与配置

**Cargo.toml 依赖**
- `axum` 0.7 + `tower` + `tower-http`
- `tokio` 1（full）
- `sqlx` 0.8（mysql / runtime-tokio-rustls）
- `aws-sdk-s3` 1（含 rustls / behavior-version-latest）
- `sha2` 0.10 + `tokio-stream` + `futures-io`
- `serde` / `serde_json`
- `tracing` / `tracing-subscriber`
- 与 api-gateway / token-server 保持一致

**config.yaml**
```yaml
server:
  http_addr: "0.0.0.0:8084"

mysql:
  url: "mysql://fs:fs@mysql:3306/file_storage"

s3:
  endpoint: "http://rustfs:9000"
  region: "us-east-1"
  bucket: "files"
  access_key_env: "RUSTFS_ACCESS_KEY"
  secret_key_env: "RUSTFS_SECRET_KEY"
  presign_ttl_secs: 300

consul:
  addr: "consul:8500"
  service_name: "file-storage-service"

backup:
  enabled: false
  schedule: "0 2 * * *"
  data_path: "/var/rustfs-data"
  mysql_dump: true
  restic_repo_env: "RESTIC_REPOSITORY"
  restic_password_env: "RESTIC_PASSWORD"
  keep_daily: 7
  keep_weekly: 4
  keep_monthly: 6

worker:
  orphan_scan_secs: 300
  reconcile_cron: "0 3 * * *"
```

**目录结构**
```
services/file-storage-service/
├── Cargo.toml
├── Dockerfile
├── config/
│   └── config.yaml
├── migrations/
│   └── 2026_06_10_init.sql
└── src/
    ├── main.rs
    ├── config.rs
    ├── db.rs
    ├── s3.rs
    ├── hasher.rs
    ├── handlers.rs
    ├── service.rs
    ├── auth.rs
    ├── orphan_worker.rs
    ├── reconcile.rs
    └── backup.rs
```

**RustFS 部署**（`infrastructure/docker-compose.yml` 新增）
```yaml
rustfs:
  image: rustfs/rustfs:latest
  volumes:
    - rustfs-data:/var/rustfs-data
  ports:
    - "9000:9000"
  environment:
    RUSTFS_ACCESS_KEY: ${RUSTFS_ACCESS_KEY}
    RUSTFS_SECRET_KEY: ${RUSTFS_SECRET_KEY}

file-storage-service:
  build: ../services/file-storage-service
  depends_on: [mysql, rustfs, consul]
  environment:
    RUSTFS_ACCESS_KEY: ${RUSTFS_ACCESS_KEY}
    RUSTFS_SECRET_KEY: ${RUSTFS_SECRET_KEY}
    RESTIC_REPOSITORY: ${RESTIC_REPOSITORY:-}
    RESTIC_PASSWORD: ${RESTIC_PASSWORD:-}
```

**路由配置**（api-gateway 新增 `file-storage.router.toml`，前缀 `/api/files`）

---

## 9. 后续可演进（不在本次范围）

- 分块 CDC 去重（节省大文件内部重复）
- 多节点 RustFS 集群 / 副本跨机
- 病毒扫描钩子
- 图片缩略图 / 视频转码
- 软删除 + 回收站
- 配额管理（按 tenant / owner 限容量）
