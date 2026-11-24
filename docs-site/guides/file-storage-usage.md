# LMMS 文件服务与文件存储使用规范

> 状态：基于当前仓库实现的使用说明与能力盘点（2026-11-24）
> 适用对象：业务服务、前端集成、SDK 使用者、AI Agent、运维人员
> 代码事实优先：`services/file-service/**`、`contracts/http-conventions.md`、`services/file-service/openapi.yaml`、`sdk/file-services/**`

## 1. 先明确：LMMS 提供的是文件对象服务，不是通用文件系统

LMMS 当前的 `file-service` 是集中式文件对象管理服务。它负责文件内容的存储、文件元数据、上传/下载和临时访问授权。

它**不等同于** POSIX 文件系统或网络共享盘。当前接口没有提供任意目录树、目录创建、目录重命名、文件移动/复制、文件锁、挂载、文件版本历史等通用文件系统语义。业务系统不应把 `storage_path` 当作可直接访问的本机路径，也不应绕过 file-service 直接读写存储目录或对象桶。

建议统一使用术语：
- **文件对象（File Object）**：由 file-service 管理的一份文件内容及其元数据。
- **对象存储（Object Storage）**：文件字节实际落盘的位置，可为本地目录或 S3 兼容存储。
- **文件元数据（File Metadata）**：数据库中的文件 ID、名称、大小、hash、租户/用户归属、临时标记及时间戳。
- **预签名 URL（Presigned URL）**：在限定操作和有效期内访问对象存储的临时凭证。

## 2. 当前架构

```text
用户 / 前端
    |
    v
业务服务 API（用户身份、业务归属与授权的责任边界）
    |
    | service-jwt, aud=file-service
    v
file-service
    |                         |
    | 文件元数据              | 文件字节
    v                         v
MySQL / PostgreSQL       ObjectStore
                         |-- local: STORAGE_PATH
                         |-- rustfs: S3-compatible
                         |-- s3: AWS / Aliyun OSS / 其他兼容服务
```

关键约束：
1. `file-service` 是内部服务；按照 `contracts/http-conventions.md` 的 2026-11-07 契约，所有 `/file/*` 端点都要求 `aud=file-service` 的 service-jwt。
2. 用户 JWT 不能直接调用 file-service；api-gateway 不再暴露 `/v1/file/*` 用户路由。
3. 用户面操作由业务服务提供接口。业务服务校验用户权限和业务归属后，使用 service-jwt 调用 file-service。
4. S3-compatible 后端的文件下载/直传可通过预签名 URL 交给对象存储执行；签名 URL 本身是 bearer capability，应按敏感凭证保护。LocalStorage 不支持原生预签名 URL，服务明确返回 501，不生成无法验证的伪签名代理 URL。
5. 元数据数据库与对象存储是两个独立持久化面；调用方不得假设两者天然具备跨系统事务。

## 3. 当前能力清单

| 能力 | 当前状态 | 使用说明 |
|---|---|---|
| 上传文件 | 已有 | multipart：`metadata` JSON + `file` 字节；默认上限 100 MiB |
| 文件列表 | 已有 | 分页，可按 tenant、user、filename 查询 |
| 查询元数据 | 已有 | 通过文件 ID 查询 |
| 下载文件 | 已有 | 服务端下载接口；仅内部服务身份可调用 |
| 删除文件 | 已有 | 按文件 ID 删除 |
| 预签名 GET | 已有 | 仅 S3-compatible driver 可生成对象存储 GET 签名；LocalStorage 返回 501 |
| 预签名 PUT | 已有 | 仅 S3-compatible driver 可生成 PUT 签名；业务上传流程仍需定义对象创建/覆盖约束 |
| 本地存储 | 已有 | 适合开发和单机测试，不建议多实例生产共用 |
| S3 兼容存储 | 已有 | RustFS / MinIO / AWS S3 / Aliyun OSS 等，通过配置适配 |
| 临时文件元数据 | 已有字段 | `is_temporary`、`expires_at`、`expires_in_seconds` |
| 临时文件自动清理 | 已实现 | 每 60 秒扫描、每轮最多 100 条；对象删除失败保留元数据并重试；临时文件默认 TTL 24 小时 |
| 目录树 / 文件夹 | 不支持 | 当前模型是扁平文件对象列表，不提供目录实体 |
| 移动 / 复制 / 重命名 | 不支持 | 需要新增明确 API 和一致性语义 |
| 版本历史 / 回收站 | 不支持 | 当前模型未体现版本或软删除历史 |
| 断点续传 / 分块上传 | 未确认 | 当前 SDK 文档展示单次 multipart 上传，不应假定支持 |
| 内容全文检索 / 预览 / 病毒扫描 | 未体现 | 由后续需求单独设计 |

> “已有”表示本次在 README、OpenAPI、SDK、存储抽象或服务配置中找到对应实现证据；不代表已完成生产级容量、故障恢复或安全压测。

## 4. 文件对象数据模型

当前 `file_files` 主要字段：

| 字段 | 含义 | 规则 |
|---|---|---|
| `id` | 文件对象 ID | UUID 字符串 |
| `filename` | 原始文件名 | 展示/下载名称，不作为存储路径 |
| `content_type` | MIME 类型 | 可空；下载时应安全回退 |
| `size` | 文件字节数 | 服务端实际写入后确定 |
| `hash` | SHA-256 摘要 | 可空；用于完整性/去重能力的基础，不等于当前已提供去重 |
| `storage_path` | 内部对象键/存储相对路径 | 内部字段，禁止客户端自行拼接或访问 |
| `tenant_id` | 租户归属 | 必须由已认证的业务服务按业务上下文传入 |
| `user_id` | 用户归属 | 操作发起用户；由调用业务服务负责校验 |
| `is_temporary` | 是否临时文件 | 生命周期标记 |
| `expires_at` | 过期时间 | `is_temporary=true` 时有效；未指定 TTL 默认 24 小时，由后台清理器执行删除 |
| `created_at` / `updated_at` | 创建/更新时间 | UTC 时间语义遵循平台 HTTP 约定 |

注意：
- `tenant_id` 和 `user_id` 不是由 file-service 从 service-jwt 自动推断出来的。service-jwt 的主体是调用服务；调用服务必须在鉴权后把真实业务归属写入 metadata。
- `storage_path` 属于实现细节。不得将它暴露为用户可访问的本地路径或让客户端自行指定。
- 数据库迁移历史存在类型调整；以当前 OpenAPI/DTO 与当前迁移最终状态为准，不要照抄旧 V001 的字段类型。

## 5. 存储后端选择

通过 `FILE_STORAGE_BACKEND` 在启动时选择 driver，同一服务镜像可使用不同后端。

| 后端 | 配置 | 适用场景 | 生产建议 |
|---|---|---|---|
| `local` | `STORAGE_PATH` | 本地开发、单机测试 | 单实例/临时环境；多副本部署避免使用 |
| `rustfs` | `S3_ENDPOINT` + S3 凭据 | 私有化部署、自建对象存储 | 推荐优先评估 |
| `s3` | AWS 或兼容服务 endpoint + S3 凭据 | 公有云/兼容对象存储 | 按云厂商权限、网络和生命周期策略配置 |

### 5.1 本地开发

```bash
DATABASE_URL=mysql://user:password@localhost/superpowers
FILE_STORAGE_BACKEND=local
STORAGE_PATH=./data/files
```

服务启动时会创建存储根目录。应将该目录视为持久化数据目录，不要放入临时构建目录；备份时必须同时考虑数据库和文件内容。

### 5.2 RustFS 私有化部署

```bash
DATABASE_URL=mysql://user:password@mysql/superpowers
FILE_STORAGE_BACKEND=rustfs
S3_ENDPOINT=http://rustfs:9000
S3_BUCKET=coder-files
S3_REGION=us-east-1
S3_ACCESS_KEY=<secret>
S3_SECRET_KEY=<secret>
S3_AUTO_CREATE_BUCKET=false
```

建议生产环境预先创建 bucket，并给 file-service 使用最小权限的专用凭据。不要把对象存储管理凭据发给浏览器或普通业务用户。

### 5.3 AWS S3 / Aliyun OSS 等

```bash
FILE_STORAGE_BACKEND=s3
S3_BUCKET=<bucket>
S3_REGION=<region>
S3_ENDPOINT=<optional-s3-compatible-endpoint>
S3_ACCESS_KEY=<secret>
S3_SECRET_KEY=<secret>
```

AWS 标准 endpoint 可按 region 推导；其他 S3-compatible 服务按供应商要求设置 endpoint、region 和 path-style。兼容性仍需针对目标供应商执行上传、下载、删除、预签名及过期测试。

## 6. 服务调用流程

### 6.1 上传（推荐：业务服务代理）

1. 前端请求业务服务的附件上传/业务创建接口。
2. 业务服务验证用户身份、租户、业务对象权限和文件用途。
3. 业务服务使用 `aud=file-service` 的 service-jwt 调用 file-service。
4. file-service 保存文件字节并写入元数据，返回 FileInfo。
5. 业务服务将 file ID 关联到自己的业务记录；业务记录只保存 file ID，不保存物理路径。

上传 metadata 示例：

```json
{
  "filename": "design.pdf",
  "content_type": "application/pdf",
  "tenant_id": 1,
  "user_id": "user-123",
  "is_temporary": false
}
```

### 6.2 下载

1. 前端请求业务服务的下载授权接口，提交业务记录 ID / file ID。
2. 业务服务验证当前用户是否有权访问该业务附件。
3. 业务服务通过 file-service 创建短 TTL 的 GET 预签名 URL。
4. 前端获得 URL 后直接从对象存储下载；不要把 URL 写入长期日志、业务表或分析事件。

**重要：** 当前契约明确用户 JWT 不直接访问 file-service。TypeScript SDK 是后端服务间 SDK，使用内部 `/file/*` 路径和 `aud=file-service` service-jwt，不可用于浏览器。预签名仅适用于 S3-compatible 后端；LocalStorage 创建预签名会返回 501。

### 6.3 删除

删除前由业务服务验证业务权限，并处理业务记录引用。建议先解除业务引用，再调用 file-service 删除；需要定义失败重试和孤儿文件清理策略。不要让任意用户仅凭 file ID 删除文件。

## 7. 身份、权限与安全规则

必须遵守：

- file-service 仅接受服务级 JWT，目标 audience 为 `file-service`。
- 用户级授权必须在业务服务完成；file-service 不能替业务服务判断“用户是否拥有某张合同/某个项目的附件”。
- 业务服务传入的 `tenant_id`、`user_id` 必须来自已验证的用户和业务上下文，不可直接信任前端传值。
- 每次查询、下载、删除前，业务服务必须先验证文件与业务对象的关联关系，不能仅按 file ID 放行。
- 预签名 URL 应使用最短可用 TTL；避免写入日志、埋点、错误报告或持久化业务字段。
- 对上传文件名做展示层编码；不得把文件名直接拼接成本地路径或对象键。
- 根据业务风险限制 MIME、扩展名、文件大小；不要仅信任客户端 Content-Type。
- 对公开下载、可执行内容、HTML/SVG 等主动内容采用下载附件策略或额外安全检查。
- S3 凭据只放服务端 secret 管理系统；禁止提交到 Git、镜像层或客户端构建产物。
- 本地后端必须确保 storage root 不可被非授权进程写入；当前 LocalStorage 有 canonical path containment 检查，但这不替代操作系统目录权限和隔离。

## 8. 备份、恢复与运维

文件系统数据和数据库元数据必须视为一个逻辑整体：

- 备份：同时备份 file-service 数据库中的 `file_files` 元数据和实际对象存储。
- 恢复：先恢复对象存储，再恢复/开放元数据访问；恢复后抽样校验 file ID、对象键、size 和 hash。
- 删除：必须处理“对象删除成功但 DB 更新失败”以及“DB 记录删除但对象删除失败”等部分失败情况。
- 切换后端：不是简单修改环境变量。必须迁移所有对象、校验数量/大小/hash、更新运行配置并保留回滚窗口。
- 多实例：生产环境优先共享的 S3-compatible 对象存储；不要让不同容器各自使用容器本地目录。
- 监控：至少监控上传/下载失败率、延迟、存储容量、对象数量、预签名失败、清理失败和 DB/对象不一致。

## 9. SDK 使用现状与契约注意事项

仓库有 `sdk/file-services` TypeScript SDK，包含：
- `files.upload`、`files.page`、`files.info`、`files.download`、`files.remove`
- `presign.create`

但当前仓库文档存在需要统一的契约漂移：
1. `sdk/file-services/README.md` 仍按 `/v1/file` 网关用户 API 描述，而 `contracts/http-conventions.md` 明确 2026-11-07 后 file-service 是纯内部服务，网关不再暴露这些用户路由。
2. SDK README 示例中的预签名操作值写为 `download`；服务 AGENTS 约定写 `get|put`，OpenAPI 的 operation 当前是宽泛 string。应统一为稳定枚举及文档。
3. 服务 README 的上传 curl 示例仍使用 JSON body；OpenAPI 和 SDK 实际描述的是 multipart 的 `metadata` + `file`。

在上述漂移修复前：
- 不要把当前 TS SDK 直接作为浏览器用户侧文件 API 的正式集成方案。
- 服务间调用可复用 SDK 的 HTTP/multipart/DTO 实现，但必须提供 service-jwt token，并核验 SDK base URL 和路由前缀适配。
- 任何 SDK 路由或认证语义修改必须同步 OpenAPI、契约、测试和 README。

## 10. 当前缺口与建议补齐顺序

### P0：先统一现有契约

- [ ] 统一 `file-services` SDK 的路由和认证说明，明确它是服务端 SDK 还是用户端 SDK。
- [ ] 明确预签名 operation 枚举（建议 `get` / `put`），同步 handler、OpenAPI、SDK types、测试和文档。
- [ ] 修正 file-service README 的 multipart 上传示例。
- [ ] 验证 `is_temporary/expires_at` 的清理执行器、失败重试、删除顺序和测试；若尚未实现，删除“自动清理已支持”的承诺或补齐实现。

### P1：补强文件对象生命周期

- [ ] 文件状态模型：上传中、可用、删除中、删除失败、已删除/过期。
- [ ] 对象写入与 DB 元数据之间的补偿机制，避免孤儿对象/悬空元数据。
- [ ] 幂等上传、内容摘要校验、重复上传策略。
- [ ] 对象删除失败重试、孤儿对象扫描与修复工具。
- [ ] 明确预签名 URL 的 TTL 上下限、权限、审计和撤销能力边界。

### P2：按业务需要扩展，不要提前把对象存储做成网盘

- [ ] 分块/断点续传（大文件场景）。
- [ ] 文件夹/逻辑目录（仅当业务需要时，建议以业务标签或虚拟目录实现）。
- [ ] 版本历史、回收站、保留策略。
- [ ] 病毒扫描、内容检测、缩略图/预览、全文索引。
- [ ] 统一的跨服务文件引用/附件关系模型。

## 11. 实现核验清单

新增或修改文件能力时，至少验证：

- [ ] service-jwt audience 校验；用户 JWT 被拒绝。
- [ ] 跨租户读取/删除被拒绝；调用服务不能通过伪造 tenant/user 参数越权。
- [ ] multipart 格式、0 字节、超限文件、非法 metadata、恶意文件名。
- [ ] local 路径穿越与符号链接逃逸。
- [ ] S3-compatible 后端的 put/get/stream/delete/exists/size/presign；LocalStorage 的 presign 必须返回 501。
- [ ] 预签名 URL 过期、错误操作、TTL 越界、LocalStorage 501、错误 file ID 和越权访问。
- [ ] DB 写入失败、对象写入失败、删除部分失败的补偿行为。
- [ ] 临时文件到期处理、并发清理、重复清理和清理失败重试。
- [ ] 备份恢复后文件对象和元数据一致。
- [ ] OpenAPI、SDK DTO、服务 README、用户手册同步。

## 12. 相关源码与文档

- `services/file-service/AGENTS.md`
- `services/file-service/README.md`
- `services/file-service/openapi.yaml`
- `services/file-service/src/storage/object_store.rs`
- `services/file-service/src/storage/local.rs`
- `services/file-service/src/storage/s3.rs`
- `services/file-service/src/config.rs`
- `services/file-service/src/db.rs`
- `services/file-service/migrations/`
- `contracts/http-conventions.md`
- `sdk/file-services/`

---
本文件描述的是当前仓库可核验的文件对象服务边界，不代表已实现通用文件系统。能力状态发生变化时，必须同步更新本文件及对应契约。
