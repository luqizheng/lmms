# CONVENTIONS: libs/ Crate 拆分策略

> 日期：2026-07-29
> 状态：约定文档
> 关联：[ADR-001](./ADR-001-api-gateway-multi-file-router-config.md)

## 1. 当前结构

`libs/` 目录下 17 个 crate（不含 `digua_common/` 外观 crate）：

| Crate | 职责 | 依赖方向 |
|-------|------|---------|
| `digua_common-error` | 错误码定义（从 contracts 生成） | 被所有 crate 依赖 |
| `digua_common-crypto` | 加密工具（AES、bcrypt、签名） | 独立 |
| `digua_common-validator` | 输入校验（email、phone、password） | 依赖 digua_common-error |
| `digua_common-config` | 配置加载（TOML / env） | 独立 |
| `digua_common-log` | 日志初始化（tracing subscriber） | 独立 |
| `digua_common-db` | 数据库连接池 + 查询构建（sqlx） | 依赖 digua_common-error、digua_common-config |
| `digua_common-cache` | Redis 缓存抽象 | 依赖 digua_common-error、digua_common-config |
| `digua_common-jwt` | JWT 签发/验签/解析 | 依赖 digua_common-crypto、digua_common-error |
| `jwt-ctx` | JWT 上下文 extractor（Principal、TenantScope） | 依赖 digua_common-jwt、digua_common-error |
| `digua_authz` | 权限码定义 + RBAC 检查 | 依赖 digua_common-error |
| `digua_common-http` | HTTP 客户端（ResilientClient 韧性三件套） | 依赖 digua_common-error、digua_common-config |
| `digua_common-ratelimit` | 令牌桶限流（api-gateway 使用） | 依赖 digua_common-cache |
| `digua_common-event` | 事件 envelope + outbox 持久化模型 | 依赖 digua_common-error |
| `digua_common-telemetry` | OTel tracing 初始化 | 独立 |
| `digua_common-telemetry-axum` | Axum tracing middleware | 依赖 digua_common-telemetry |
| `digua_common-health` | /health 端点标准实现 | 独立 |

> `digua_common/` 是外观 crate（re-export 常用子 crate），不新增逻辑。

## 2. 拆分原则

### 2.1 什么时候拆

满足以下 **任意一条** 即可拆为独立 crate：

- **编译隔离**：某个模块的编译时间显著拖慢全 workspace（如 `digua_common-db` 依赖 `sqlx` macro，编译最慢）
- **依赖方向**：A 依赖 B，但 B 不应依赖 A。把不需要 B 的类型拆出去（如 `jwt-ctx` 从 `digua_common-jwt` 独立）
- **使用面窄**：仅 1-2 个服务使用（如 `digua_common-ratelimit` 仅 api-gateway 用），不应强制全量引入

### 2.2 什么时候不拆

- **使用面广 + 体积小**：如 `digua_common-crypto`、`digua_common-log`——每个服务都需要，拆分反而增编译次数
- **高频共变**：每次改 A 必然改 B（如 `digua_common-telemetry` 和 `digua_common-telemetry-axum`）。但当前拆了是因为依赖链不同（axum 额外依赖 `axum` crate）
- **不到 200 LOC**：除非有 §2.1 的编译隔离需求

## 3. 依赖方向图

```
digua_common-error  ←── digua_common-validator
              ←── digua_common-crypto ──→ digua_common-jwt ──→ jwt-ctx
              ←── digua_common-db
              ←── digua_common-cache
              ←── digua_common-http
              ←── digua_common-event
              ←── digua_authz
digua_common-config ←── digua_common-db
              ←── digua_common-cache
              ←── digua_common-http
digua_common-cache  ←── digua_common-ratelimit
digua_common-telemetry ←── digua_common-telemetry-axum
```

**无循环依赖**。`digua_common-error` 是唯一被全量依赖的叶子节点。

## 4. 版本策略

### 4.1 当前（workspace path 依赖）

所有 crate 用 `path = "../digua_common-xxx"` 依赖。Workspace 内所有改动瞬间生效，无需独立版本号。

### 4.2 未来（发布到 kellnr 后）

当需要发到私有 registry 时：

| 策略 | 说明 |
|------|------|
| **Semver 策略** | 沿用 `cargo` 惯例：`0.y.z` 表示 unstable，minor bump 可 breaking |
| **版本同步** | 不要求 17 个 crate 同版本号。仅当 API 变化时 bump |
| **发布顺序** | 按拓扑顺序发布（leaf → root）：先发 `digua_common-error`、`digua_common-crypto`，再发依赖它们的 crate |
| **自动化** | `coder release publish rust --targets libs` 已处理拓扑排序 + path→version 依赖替换 |

## 5. 已知技术债

| 问题 | 风险 | 建议 |
|------|------|------|
| `kudi-http-client` vs `digua_common-http::ResilientClient` 功能重叠 | SDK 路径与服务调用路径互串 | 已在 SDK spec 区分，代码层暂未强制隔离。2.0 后用 trait 做编译期隔离 |
| `digua_common/` 外观 crate 职责模糊 | re-export 覆盖不全，新 crate 加进来时容易忘记更新 | 不阻断 1.0，等 kellnr 发布前统一检查 |

## 6. 变更指南

### 新增 crate

1. 新建 `libs/<name>/Cargo.toml` + `src/lib.rs`
2. 在 workspace `Cargo.toml` `members` 中注册
3. 如果设计上属于 "digua_common" 系列，更新 `libs/digua_common/Cargo.toml` re-export
4. 遵循本约定 §2.1 判断是否值得拆

### 合并 crate

1. 确认无循环依赖问题
2. 合并 `Cargo.toml` 依赖项（union）
3. 更新所有引用的 `use` 路径
4. 更新 `libs/digua_common/Cargo.toml` 如有 re-export

### 废弃 crate

1. 确认 0 个下游依赖（rust-analyzer "Find All References"）
2. 从 workspace `members` 移除
3. 删除目录（或移动到 `libs/_archived/` 保留记录）
