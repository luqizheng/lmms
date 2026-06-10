# ADR-001: API Gateway 多文件路由配置 + 重写规则链 + 手工热重载

## Status
Accepted

## Date
2026-06-08

## Context
`api-gateway` 路由集中在 `docker/config/api-gateway/api-gateway.toml` 一条 `[[routes]]` 列表里。当 URL 转发与重写规则增多时，单文件膨胀且难以按 URL 维度维护；按服务或业务域聚合又与「请求路径在前缀级匹配」的实际排错心智不一致。

约束：
- 大量 URL 转发（strip 前缀、改写 path 参数）
- 偶尔需要重定向与 header 注入
- 配置改动不希望强制重启进程
- 现有 Consul 服务发现链路不动
- Windows + Linux 双平台部署

## Decision
采用「按 URL 前缀拆分的多文件配置 + 重写规则链 + 手工触发的 `validate` / `reload` 命令」三件套：

1. **路由分文件**：每个前缀一个 `routes/<name>.router.toml`，文件名字典序合并；主 `api-gateway.toml` 只保留全局 server/redis/jwks/consul。
2. **重写规则链**：每条路由可声明有序的 `rewrite = [{type=..., ...}]`，支持 `strip_prefix` / `redirect` / `header_set` / `header_add` / `path_param`，转发前顺序执行，`redirect` 短路返回 30x。
3. **手工热重载**：仿 nginx `nginx -t` / `nginx -s reload`：
   - `api-gateway validate`：仅校验路由目录，退出码 0/1
   - `api-gateway serve`：启动时加载，运行时通过 SIGHUP（POSIX）或 `POST /__admin/reload`（HTTP）触发；`ArcSwap<RouteTable>` 原子切换，校验失败保留旧表

实现要点：
- `RouteConfig` 字段 `prefix` → `match_pattern`（TOML 键 `match`），支持 `/**` 通配
- `RewriteSpec` 用 `#[serde(tag = "type")]` 枚举，未知类型 → 启动/重载报错
- `AppState.route_table: Arc<ArcSwap<RouteTable>>`，handler 每请求 `.load()` 取当前快照

## Alternatives Considered

### 路由：单文件 vs 按服务 vs 按业务域 vs 按前缀
- **单文件**（原方案）：最简单，5 条路由无压力；缺点是 URL 维度排错时需在大文件里搜索。
- **按服务**（`user-service.router.toml` 聚合 users/roles/permissions）：以后端服务为粒度，但跨服务的 URL 规则（如 `/api/auth/**` + `/api/users/**` 同时改造）会被切散。
- **按业务域**（`iam.router.toml` 聚合 users/roles/permissions/auth）：业务概念对齐，但 IAM 边界随产品变化需要频繁拆并文件。
- **按前缀**（采用）：与运行时匹配模型一致——`/` 后第一段就是路由表第一维，文件即目录心智模型。冲突检测（match_pattern 重复）也最直观。

### 重写表达：内联字段 vs 规则链 vs nginx 风格
- **内联字段**（`strip_prefix = "/api"`）：TOML 原生结构、零学习成本；但多条规则或顺序敏感场景表达力差。
- **规则链**（采用）：`rewrite = [...]` 数组 + `type` tag；扩展性最强，新增规则类型只改 `RewriteSpec` enum + executor，serde 自动反序列化。
- **nginx 风格 location 块**：行业熟悉，但 TOML 无嵌套块语法，需要发明微型 DSL；ROI 低。

### 热更新：文件 watcher vs 手工命令 vs consul-kv
- **文件 watcher**（`notify` crate）：开发体验好，但与「git pull 配置文件 → 验证 → 切换」的生产流程不匹配；编辑器多次保存抖动需要 debounce；非 Unix 平台行为差异。
- **手工命令**（采用 `validate` + `serve --reload` + SIGHUP + admin endpoint）：与 nginx 运维心智一致；reload 前可独立验证；失败回滚到旧表是显式行为而非默认。
- **Consul KV 推送**：跨实例统一，但引入额外 KV 一致性问题与权限模型；本项目 Consul 仅用于服务发现，再扩边界会模糊职责。

## Consequences

### 正面
- 路由变更粒度从「改大文件 + 重启」降到「加一个 `*.router.toml`」
- 复杂 URL 转发可在 `rewrite` 链里组合，无需改 Rust 代码
- 路由校验可在 CI / 部署前 `api-gateway validate` 跑通
- 热重载不重启进程，旧请求不受影响

### 负面 / 代价
- 新增 `arc-swap` / `clap` / `toml` / `glob` / `tempfile` 5 个直接依赖，编译时间略增
- `RouteConfig` 字段从 `prefix` 改名 `match_pattern`，所有引用方（middleware / main.rs / tests）已同步迁移，未来不要回滚
- 手工 reload 需要运维记得触发；文件 watcher 那种「保存即生效」的体验没了
- Windows 不支持 SIGHUP，只能走 `POST /__admin/reload`——文档需明示

### 后续可能演进
- 若运维反复忘记 reload，可加 systemd path unit 或 k8s ConfigMap watcher 触发 admin endpoint（不引入 notify 到 gateway 进程）
- `RewriteSpec` 新增 `rewrite_host` / `set_query` 等规则时只需扩 enum + executor，DSL 不变
