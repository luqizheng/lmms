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