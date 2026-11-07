# HTTP 约定

## 响应格式

所有 API 响应统一 JSON 结构：

```json
{
  "code": "IAM_AUTH_1001",
  "message": "invalid credentials",
  "data": null,
  "request_id": "abc-123"
}
```

成功响应 `code` 固定为 `"OK"`，`data` 携带业务数据。

## 分页格式

```json
{
  "code": "OK",
  "message": "success",
  "data": {
    "items": [...],
    "total": 100,
    "page": 1,
    "size": 20
  },
  "request_id": "..."
}
```

## 排序约定

- 排序参数：`sort_by`（字段名） + `sort_order`（`asc` / `desc`）
- 默认：`created_at desc`

## 时间格式

- 所有时间字段：ISO8601 UTC，例 `2026-06-13T10:30:00Z`
- 字段命名：`*_at`（如 `created_at`、`updated_at`）

## ID 格式

- 所有 ID 使用 `String` 类型（雪花 i64 转 string）
- 前端不处理大整数精度问题
## 服务间身份与文件服务契约（2026-11-07 裁决）

### file-service 是纯内部服务

`file-service` 的 `/file/*` 全部端点（含 `/file/presign`）由
`verify_aud_principal_middleware("file-service")` 门禁：只接受 service-jwt
（`POST /tokens/service-jwt` 铸造，`aud=file-service`）。用户 JWT
（`aud=coder-micro-services`）一律 401，网关不再暴露 `/v1/file/*` 用户路由。

- 用户面入口 = 业务服务接口（如合同创建接口内含附件），由业务服务经
  `ServiceJwtCache.get("file-service")` 出站调用（范例：notification-service `iam_client.rs`）。
- 下载通道 = 对象存储 presign 签名 URL（自带签名+过期，用户免 JWT 直接访问）；
  presign 的**创建**动作同样只允许服务调用。

### 归属身份由调用方显式声明

file-service 无法校验业务归属，也不从 token 推断用户身份（service-jwt 的
`sub` 是调用服务名）。真实用户归属由受信调用方在请求体中显式传递：

- 上传：`UploadFileMeta.user_id`（操作发起用户）、`tenant_id`；
- 查询/删除等按既有 `QueryFiles.user_id` / 路径参数。

责任划分：**调用方对声明的 user_id 的正确性负责**（它在 aud 门内，已通过
bootstrap 换发链自证）；file-service 只验调用方，记账照单全收并写审计。
