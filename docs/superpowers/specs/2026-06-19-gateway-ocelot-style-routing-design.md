# API Gateway Ocelot 风格路由配置设计

> 日期：2026-06-19
> 范围：`services/api-gateway` 路由配置语法 + 加载/匹配/转发实现 + 全部 router.toml 迁移

---

## 一、背景与目标

现行网关使用 `match` + `rewrite[]` 配置，rewrite 类型多达 6 种（strip_prefix / add_prefix / redirect / header_set / header_add / path_param），心智成本高，重复样板多。

目标：参考 Ocelot 风格，统一为 `upstream` / `downstream` 两字段表达"网关入口路径 → 微服务路径"映射，让配置更声明式、更易读、更易校验。

非目标：
- 不改变前端 axios 调用路径
- 不改变后端服务的内部路由前缀
- 不改变服务发现机制（仍走 Consul）

---

## 二、最终配置语法

### 2.1 字段表

| 字段 | 必填 | 类型 | 说明 |
|---|---|---|---|
| `upstream` | 是 | string | 网关入口模板（前端到达网关的路径，已剥除 `/api`） |
| `downstream` | 是 | string | 转发到微服务的路径模板（含 query） |
| `service` | 是 | string | Consul 服务名（如 `iam-service`） |
| `public` | 否 | bool | true 跳过 JWT 校验，默认 false |
| `ws` | 否 | bool | true 升级 WebSocket，默认 false |
| `headers` | 否 | table | 转发时附加请求头，如 `{ "X-Source" = "gateway" }` |

废弃字段：`match`、`rewrite`、`strip_prefix`。

### 2.2 占位符语法

占位符统一以 `{` 与 `}` 包裹变量名，无 catch-all / 段限定区分。

原理：
1. 把 upstream 模板拆成 `[字面量, {var}, 字面量, {var}, ...]` 序列
2. 用相邻字面量在请求 url 上做锚定切割，把每个 `{var}` 之间的字符串提取到 dict
3. 对 downstream 模板做朴素 `{name}` → `dict[name]` 字符串替换

约束：
- `{name}` 之间必须有非空字面量分隔（否则锚定不唯一）
- 变量值可包含 `/`（即可跨段，等价旧 catch-all）
- 同名变量在 upstream 内只允许出现一次
- query 占位与 path 占位共享同一 dict，无差异处理

示例：

| upstream | downstream | 请求 url | dict | 转发 url |
|---|---|---|---|---|
| `/v1/iam/{anything}` | `/iam/{anything}` | `/v1/iam/users/42/roles` | `{anything: users/42/roles}` | `/iam/users/42/roles` |
| `/v1/iam/{anything}` | `/iam/{anything}` | `/v1/iam/auth/login` | `{anything: auth/login}` | `/iam/auth/login` |
| `/v1/billing/units/{sub}/{unit}/updates` | `/billing/subs/{sub}/updates?unitId={unit}` | `/v1/billing/units/s1/u9/updates` | `{sub: s1, unit: u9}` | `/billing/subs/s1/updates?unitId=u9` |

### 2.3 示例

```toml
# 公共登录路由
[[route]]
upstream   = "/v1/iam/auth/login"
downstream = "/iam/auth/login"
service    = "iam-service"
public     = true

# 通配受保护路由
[[route]]
upstream   = "/v1/iam/{anything}"
downstream = "/iam/{anything}"
service    = "iam-service"

# WebSocket
[[route]]
upstream   = "/v1/notification/ws"
downstream = "/notification/ws"
service    = "notification-service"
ws         = true

# query 双向映射
[[route]]
upstream   = "/v1/billing/units/{sub}/{unit}/updates"
downstream = "/billing/subs/{sub}/updates?unitId={unit}"
service    = "billing-service"

# 附加 header
[[route]]
upstream   = "/v1/audit/{anything}"
downstream = "/audit/{anything}"
service    = "audit-service"
headers    = { "X-Source" = "gateway" }
```

---

## 三、内部模型

### 3.1 配置反序列化（`config.rs`）

```rust
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RouteConfig {
    pub upstream: String,
    pub downstream: String,
    pub service: String,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub ws: bool,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}
```

删除：`RewriteSpec` 枚举、`RouteConfig.match_pattern`、`RouteConfig.rewrite`。

### 3.2 编译产物（`routes_loader.rs`）

```rust
pub struct CompiledRoute {
    pub up_template: Template,    // 用于提取 dict
    pub down_template: Template,  // 用于渲染 path+query
    pub var_names: Vec<String>,
    pub service: String,
    pub public: bool,
    pub ws: bool,
    pub headers: Vec<(HeaderName, HeaderValue)>,
}

/// 模板：字面量与变量交替
pub struct Template {
    pub parts: Vec<TplPart>,
}

pub enum TplPart {
    Literal(String),
    Var(String),
}
```

启动期把所有 `*.router.toml` 编译为 `Vec<CompiledRoute>`，按"字面量长度降序"排序（更具体的路由优先匹配），写入 `RouteTable`。

### 3.3 匹配与提取算法

输入：请求 url（含 query），upstream `Template`。

伪码：

```
fn extract(url, tpl) -> Option<HashMap<name, value>> {
    let mut cursor = 0
    let mut dict = {}
    for (i, part) in tpl.parts.enumerate() {
        match part {
            Literal(s) => {
                if !url[cursor..].starts_with(s): return None
                cursor += s.len()
            }
            Var(name) => {
                let next_lit = tpl.parts[i+1..].first_literal()  // 可能为 None（结尾变量）
                let end = match next_lit {
                    Some(lit) => url[cursor..].find(lit)? + cursor,
                    None      => url.len(),
                }
                dict[name] = url[cursor..end]
                cursor = end
            }
        }
    }
    if cursor == url.len() { Some(dict) } else { None }
}
```

要求：
- upstream 必须以 `Literal` 开头（即 `/`）
- 两个 `Var` 之间必须夹一个非空 `Literal`，否则编译期报错
- 首匹配命中即停止

### 3.4 渲染算法

```
fn render(tpl, dict) -> String {
    tpl.parts.map(|p| match p {
        Literal(s) => s,
        Var(name)  => dict[name],   // 启动期已校验存在
    }).join("")
}
```

- path 与 query 在同一个 downstream 模板里渲染（出现 `?` 之后视为 query 段）
- header 在路由级别配置，不参与模板渲染

---

## 四、校验规则（`cli.rs validate`）

启动期与 `cargo run -p api-gateway -- validate` 共用：

1. upstream / downstream 必须以 `/` 开头
2. upstream 中两个 `{var}` 之间必须有非空字面量分隔
3. upstream 中变量名集合 == downstream 中变量名集合（双向无遗漏、无未定义）
4. upstream 内变量名不可重复
5. 同一路由文件内 upstream 不可重复
6. `headers` 中 name/value 必须是合法 HTTP 头

任何违规 → 启动失败 / `validate` 退出码非零。

---

## 五、改动清单

| 文件 | 改动 | 说明 |
|---|---|---|
| `services/api-gateway/src/config.rs` | 重写 `RouteConfig`，删 `RewriteSpec` | 见 3.1 |
| `services/api-gateway/src/routes_loader.rs` | 新增 compile/normalize 模块 | 见 3.2 |
| `services/api-gateway/src/rewrite.rs` | 删除旧 6 种 rewrite，替换为 `apply_template(req, route, vars)` | |
| `services/api-gateway/src/router.rs` | 转发前调用渲染，写回 uri 与 headers | |
| `services/api-gateway/src/cli.rs` | `validate` 命令对接新规则 | |
| `docker/config/api-gateway/routes/*.router.toml` | 全部 8 个文件改写 | iam / token / audit / notification / config / file / user / roles / permissions / prue_backend |
| `services/api-gateway/tests/**` | 改测试 fixtures 与断言 | |
| `.trae/rules/routing-rules.md` | 重写 §2.2 / §2.3 / §2.5 / §2.6 / §2.7 | |
| `.trae/rules/project_rules.md` | 删除 rewrite 类型白名单段落 | |
| `docs/guides/api-gateway-routing.md` | 同步用户文档 | |

---

## 六、迁移示例

### 6.1 旧

```toml
[[route]]
match = "/v1/iam/auth/login"
service = "iam-service"
public = true
rewrite = [{ type = "strip_prefix", prefix = "/v1" }]
```

### 6.2 新

```toml
[[route]]
upstream   = "/v1/iam/auth/login"
downstream = "/iam/auth/login"
service    = "iam-service"
public     = true
```

### 6.3 通配迁移

旧：

```toml
match = "/v1/iam/auth/**"
rewrite = [{ type = "strip_prefix", prefix = "/v1" }]
```

新：

```toml
upstream   = "/v1/iam/auth/{anything}"
downstream = "/iam/auth/{anything}"
```

---

## 七、错误处理

| 阶段 | 异常 | 行为 |
|---|---|---|
| 启动 | TOML 反序列化失败 | 进程退出，日志打印路径与字段 |
| 启动 | 校验失败（§4） | 进程退出，列出所有错误 |
| 运行 | 无路由匹配 | 404 |
| 运行 | 服务发现失败 | 503 |
| 运行 | 上游连接失败 | 502 |
| 运行 | header 解析失败 | 500（理论上启动期已拦截） |

---

## 八、测试覆盖

### 8.1 单元

- 反序列化：合法 / 缺字段 / 多余字段
- 编译：变量名一致性、相邻变量需字面量分隔、重复 upstream、变量名重复
- 提取：
  - 单变量 `/v1/iam/{anything}` 命中 / 未命中
  - 多变量 `/a/{x}/b/{y}` 提取
  - 变量值含 `/`（跨段）
  - 末尾变量 / 中间变量
- 渲染：path 单变量、多变量、含 query 的 downstream
- 校验：每条规则一个 fail case

### 8.2 集成

- 每个服务一条新写法路由 → 真实代理 200
- public=true 路由跳过 JWT
- ws=true 路由升级成功
- headers 注入生效（后端 echo 验证）

### 8.3 兼容性

- 启动期碰到旧字段（match/rewrite）→ 立即报错，提示迁移
- e2e 测试（如 `tests/e2e/config.e2e.test.ts`）调用的 url 不变，整体 pass

---

## 九、回滚策略

- 单一 commit 完成全部改动
- 保留 git tag `pre-ocelot-routing` 便于回滚
- 旧配置文件全文随 commit 改写，无双轨期

---

## 十、上线后续

1. 文档：`docs/guides/api-gateway-routing.md` 重写
2. 规则：`.trae/rules/routing-rules.md` 同步
3. CI：`coder-route-validate` skill 沿用，仅校验命令实现升级
4. 团队同步：commit message 写明 BREAKING CHANGE
