# config-services

lmms config-service SDK：**命名空间 / 配置项管理、导入导出、配置监听**的 TypeScript 客户端。零运行时依赖（原生 `fetch`），浏览器与 Node ≥18 通用，面向 api-gateway 的 `/v1/config` 路径。

与 `iam-services` 同款模式（SDK 位置例外见 `sdk/README.md`）；安装与 `.npmrc` 配置见该包 README。

## 快速上手

```ts
import { createConfigServices } from "config-services";

const cfg = createConfigServices({
  baseUrl: "http://127.0.0.1:3000",
  getToken: () => localStorage.getItem("access_token") ?? undefined,
});

// 配置项（分页，需鉴权；秘密项返回 value_masked=true + value="********"）
const items = await cfg.items.page({ namespace_id: "ns-1", key: "db." });
await cfg.items.create({ namespace_id: "ns-1", key: "db.url", value: "postgres://..." });

// 导入/导出（text/plain 承载 JSON 映射）
const text = await cfg.transfer.exportConfig("ns-1");
const map = JSON.parse(text);
await cfg.transfer.importConfig("ns-1", { "db.url": "postgres://..." });
```

## API 一览

| 模块 | 方法 | 端点 |
|---|---|---|
| `namespaces` | `page` / `get` / `create` / `update` / `remove` | `/v1/config/namespace*` |
| `items` | `page` / `get` / `create` / `update` / `remove` | `/v1/config/item*` |
| `transfer` | `exportConfig` / `importConfig` / `watch` | `/v1/config/export|import|watch` |

## 错误模型

统一抛 `ConfigServicesError`（`status` / `message`）。config 路由处理器的错误体是**纯文本**（如 `"config item not found"`），网关层同为纯文本。

## 陷阱

1. **导入/导出体是 text/plain 承载的 JSON 映射**（非 application/json），SDK 已按此处理。
2. **`watch` 上游是长轮询 stub**：睡眠 `timeout`（默认 30s）后恒返回 `{changed:false, data:[]}`——勿据此做配置推送，等上游实现 SSE 再用。
3. 秘密项读取需要 `CFG_CONFIG_REVEAL` 权限，否则 `value_masked=true` 且值为掩码。
4. 全部端点需要鉴权；`/v1/config/health` 是唯一公开路由。

## 开发

```bash
pnpm --filter config-services test / typecheck / build
```

发布走仓库统一管线（见 `AGENTS.md`「npm 包发布规则」）。
