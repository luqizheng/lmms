# 平台配置模型 — env / yaml / fail-fast 三段瀑布

> 适用：`iam-service`、`notification-service`、`token-service`、`audit-service`、`config-service`、`file-service`、`prue_backend` 共 7 个业务服务。
>
> 本文档是配置加载语义的**唯一权威说明**；各服务的 `*-env.md` 只列字段。

## 核心原则

```
最终生效值 = env 值 if 存在 else yaml 值 if 存在 else (fail fast)
```

**没有 hardcode fallback 兜底敏感字段**。遗漏 `DATABASE_URL` / `REDIS_URL` 不会让服务静默连到本地 dev DB —— 而是立即 panic 列出缺失字段。

## 三种加载形态

### 形态 A：env > yaml(seed) > fail-fast
**适用**：`iam-service`、`iam-outbox-publisher`、`notification-service`

- yaml 文件首启时若不存在 → 用 `include_str!` 内嵌的 `config/config.default.yaml` 原子写入（`create_new` race-safe）
- 敏感字段（`database.url` / `redis.url`）在模板里留空
- `validate_all_fields_present` 在合并后校验

### 形态 B：env + yaml > fail-fast（`common_config::Loader`）
**适用**：`token-service`

- `Loader::new("TOKEN").with_file(...).required(false)`，文件缺席不报错
- `Config::default` 把敏感字段留空，`validate_all_fields_present` 把关
- env 前缀 `TOKEN_`，如 `TOKEN_DATABASE_URL`

### 形态 C：纯 env（无 yaml）
**适用**：`audit-service`、`config-service`、`file-service`、`prue_backend`

- `AppConfig::from_env() -> anyhow::Result<Self>`
- `DATABASE_URL` 缺失或空白直接 `bail!`
- 其余字段有非敏感 operational default

## dev 与分发包两侧挂载

| 环境 | 挂载路径 | 配置文件位置 |
|------|---------|-------------|
| dev（`infrastructure/docker-compose-services.yml`） | `../services/<svc>/config:/etc/<svc>`（iam/notification 无 `:ro`） | 宿主机 `services/<svc>/config/`（seed 产物 gitignore） |
| 分发包（release.sh sed 后） | `./config/<slug>:/etc/<svc>` | 分发包内 `infrastructure/config/<slug>/` |

slug 映射（`release/dist-template/scripts/release.sh::SERVICE_SLUGS`）：

| 服务 | slug | 分发包内路径 |
|------|------|-------------|
| `iam-service` | `iam` | `infrastructure/config/iam/` |
| `notification-service` | `notification` | `infrastructure/config/notification/` |
| `token-service` | `token` | `infrastructure/config/token/` |

## 各服务 env 一览（敏感字段）

| 服务 | `DATABASE_URL` | `REDIS_URL` | 其它敏感 |
|------|---------------|-------------|---------|
| iam-service | **必填** | **必填** | — |
| notification-service | **必填** | **必填** | — |
| token-service | **必填**（`TOKEN_DATABASE_URL`） | — | `TOKEN_JWT_PRIVATE_KEY_PATH` / `TOKEN_JWT_PUBLIC_KEY_PATH` / `TOKEN_ISSUER` / `TOKEN_USER_SERVICE_URL` |
| audit-service | **必填** | 否（best-effort 风控，连不上仅 warn） | — |
| config-service | **必填** | — | — |
| file-service | **必填** | — | — |
| prue_backend | **必填** | — | — |

## 新增服务的接入指引

新服务应采用**形态 A**（iam/notification 的样板），步骤：

1. 在 `services/<svc>/config/config.default.yaml` 写权威模板（敏感字段空占位）
2. 在 `src/config.rs` 里：
   - `const DEFAULT_YAML: &str = include_str!("../config/config.default.yaml");`
   - `seed_if_absent` 用 `create_new` 写
   - `apply_env_overrides(&mut cfg, env_lookup)` 纯函数式覆盖
   - `validate_all_fields_present(&cfg)?` fail-fast
3. compose 挂载 `../services/<svc>/config:/etc/<svc>`（不加 `:ro`）
4. `release/dist-template/scripts/release.sh` 的 `SERVICE_SLUGS` 加一行 `<svc>=<slug>`
5. `.gitignore` 加 `services/<svc>/config/config.yaml`

参考实现：`services/iam-service/src/config.rs`（13 个单测覆盖 race / fail-fast / env 覆盖 / seed 幂等）。

## 失败语义速查

| 场景 | 行为 |
|------|------|
| yaml 不存在 + 允许 seed | 写入内嵌 default → 继续 |
| yaml 存在但敏感字段空 + env 未设 | 启动失败，列出缺失字段 |
| 数字 env 写错（如 `SERVER_PORT=abc`） | warn 保留 yaml 值，不 crash |
| 挂载 `:ro` 导致 seed 失败 | warn 后用内嵌 default 作 yaml 输入，继续 |
| 主服务与 sidecar 并发启动 | `create_new` 保证 seed 只执行一次 |
