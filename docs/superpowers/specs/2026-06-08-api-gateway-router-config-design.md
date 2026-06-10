# API Gateway 多文件路由配置 设计

日期：2026-06-08
范围：`services/api-gateway/`、`docker/config/api-gateway/`

## 背景

当前 `api-gateway.toml` 单文件集中维护 5 条 `[[routes]]`。当 URL 转发与重写规则增多时，单文件会膨胀，且难以按 URL 维度维护。本次改造将路由拆分到 `routes/*.router.toml` 多文件，并引入重写规则链与热更新能力。

## 目标

1. 路由按 URL 前缀拆分到独立 `*.router.toml` 文件
2. 每条路由支持 `rewrite` 规则链：strip_prefix / redirect / header 注入 / path_param 重写
3. 支持通配前缀 `match = "/api/auth/**"`（自动 strip 前缀）
4. 热更新：监听 `routes/` 目录变更，全量校验后原子切换
5. 移除旧 `api-gateway.toml` 的 `[[routes]]` 块

## 非目标

- 不引入 nginx 配置兼容层
- 不实现 redirect 循环检测
- 不做基于 Consul KV 的远程配置分发（仅本地文件）

## 目录结构

```
docker/config/api-gateway/
├── api-gateway.toml          # 仅 server/redis/jwks/consul 等全局配置
└── routes/
    ├── auth.router.toml
    ├── tokens.router.toml
    ├── users.router.toml
    ├── roles.router.toml
    └── permissions.router.toml
```

加载规则：扫描 `routes/*.router.toml`，按文件名字典序合并。

## DSL 结构

```toml
# auth.router.toml
[[route]]
match = "/auth/login"
service = "auth-service"
public = true

[[route]]
match = "/api/auth/**"
service = "auth-service"
public = true

[[route]]
match = "/auth/logout"
service = "auth-service"
public = true
rewrite = [
  { type = "header_set", name = "X-Auth-Audit", value = "logout" }
]
```

字段：

| 字段 | 必填 | 说明 |
|------|------|------|
| `match` | 是 | 精确路径或 `/prefix/**` 通配 |
| `service` | 是 | Consul 注册的服务名 |
| `public` | 否 | 跳过 JWT，默认 false |
| `rewrite` | 否 | 规则链，按顺序执行 |

支持的 `type`：

- `strip_prefix`：去掉路径前缀
- `redirect`：301/302 跳转，终结规则
- `header_set`：覆盖头
- `header_add`：追加头
- `path_param`：路径占位符与 query 互转

## 加载流程

启动：

1. 读取 `api-gateway.toml` 全局配置
2. glob `routes/*.router.toml`
3. 并行解析每个文件为 `Vec<RouteConfig>`
4. 合并并校验：match 唯一、service 非空、rewrite.type 在白名单
5. 校验失败 → 进程退出
6. 注入 `ArcSwap<RouteTable>`，router handler 读取快照

## 热更新机制

参照 nginx `nginx -t` / `nginx -s reload` 模式：**手工触发**校验与重载，不做文件自动监听。

CLI 子命令：

- `api-gateway validate`：扫描 `routes/`，完整解析与校验，输出每条错误，退出码 0/1。**不**触达运行时。
- `api-gateway reload`：向运行中的 gateway 进程发信号，重新解析 + 全量校验 + `ArcSwap.store(new)`。
  - 校验失败 → 保留旧表 + 打印错误，旧进程继续服务
  - 校验成功 → 原子切换，旧请求不受影响

信号方式（任选其一）：

- SIGHUP：POSIX 标准信号，Windows 用 `Ctrl+Break` 或单独实现
- 管理端口：`POST /__admin/reload`（鉴权由现有 middleware 链路处理）

依赖：`arc-swap = "1"`（不再依赖 `notify`）。

## 重写规则执行器

```rust
enum Rewrite {
    StripPrefix(String),
    Redirect { status: u16, to: String },
    HeaderSet { name: String, value: String },
    HeaderAdd { name: String, value: String },
    PathParam { from: String, to: String },
}

fn apply(rewrites: &[Rewrite], req: &mut Request) -> Result<(), Response> {
    for r in rewrites {
        match r { /* 任一失败立即返回错误响应 */ }
    }
    Ok(())
}
```

执行点：在 `forward_to_service` 构造 `target_url` 前调用。`redirect` 短路返回 30x 响应。

## 错误处理与边界

- 启动：`routes/` 不存在 → 退出；任一文件解析失败 → 退出
- 运行：reload 校验失败 → 保留旧表 + 日志
- match 冲突：启动 fail / 运行 reject
- 未知 service：保留条目，转发时由现有 discovery 逻辑返回 503
- redirect 循环：由配置者保证
- 通配前缀：`/api/auth/**` 命中后自动 strip `/api/auth` 后转发

## 测试

- `routes_loader_test.rs`：合并、冲突检测、未知 rule type
- `rewrite_test.rs`：各规则单测（strip / redirect / header / path_param）
- `validate_cli_test.rs`：合法配置退出 0；非法配置退出 1 并列出错误
- `reload_test.rs`：启动 gateway → 替换 routes/ → 触发 reload → 验证 ArcSwap 切换；非法配置触发 reload 不影响旧请求

## 迁移步骤

1. 新建 `docker/config/api-gateway/routes/` 五个文件
2. 删除 `api-gateway.toml` 中 `[[routes]]` 块
3. 更新 `services/api-gateway/src/config.rs`：拆分 RouteConfig + 加载器
4. 新增 `src/routes_loader.rs`、`src/rewrite.rs`
5. 更新 `src/router.rs`：调用 apply(rewrites)
6. 跑 `cargo test -p api-gateway`
