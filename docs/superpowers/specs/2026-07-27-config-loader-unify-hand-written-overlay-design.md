# 2026-07-27 配置加载统一为手写 env overlay

## 背景

`token-service` 启动报 `CFG_PARSE / missing field db_type`。根因:`common_config::Loader` 内部用 `config::Environment::with_prefix("TOKEN").separator("_")` 做自动 env→struct 绑定,该行为把 env 名里**每个** `_` 都当嵌套分隔符。

- 目标字段:`database.db_type`
- env 名:`TOKEN_DATABASE_DB_TYPE`
- 实际解析路径:`database.db.type` ← 与 struct 字段名不匹配 → 静默丢弃
- yaml 里 `db_type` 也没兜住,因为容器 bind mount 路径/format 组合让 yaml 层也没被加载到

`api-gateway` 用了同一个 crate 的另一半解法 —— `.separator("__")` 双下划线,把字段名里的 `_` 保住。代价是 env 名变得反直觉(`GW_SERVER__HOST` / `GW_REDIS__PREFIX`),运维手感差,且 iam-service 那份 AGENTS.md 里的"必须与 api-gateway 一致"跨服务口径变成 `REDIS_PREFIX` vs `GW_REDIS__PREFIX` 两种写法,已经出过配对错误。

其余 6 个服务(iam / notification / audit / config-service / file-service / prue_backend)都是手写映射,逐字段 `env("DATABASE_URL")` 精确取值,不受 `_` 拆分影响。**8 个服务里只有 token-service 和 api-gateway 是异类**。

## 决定

统一 8 个服务的配置加载模式:**读文件 → apply_env_overrides(手写映射) → validate**。删掉自动 env 绑定这条路径。

理由:

1. **静默失败最坏**。自动绑定拼错 env 名不报错,伪装成 "yaml missing field"。手写映射拼错就是"这个 env 没生效",可诊断。
2. **多数派已经这么做**。iam/notification/audit/config/file/prue 6 个服务已是手写,统一它俩就消除了两种"异类"。
3. **AGENTS.md 里 env 表本就是天然实现清单**。手写 apply_env_overrides 就是把那张表落到代码,还能顺带做 `parse_or_warn` 数字类兜底。
4. **少一个依赖**。`common_config::Loader` 现在唯一非平凡能力就是自动 env 绑定,把它拆了,`common-config` crate 对 token-service 不再必需(iam 早就没用了)。

## 范围

### 代码

- `services/token-service/src/config.rs`
  - 新增 `apply_env_overrides + load_config_with_env`(照 iam-service 的 shape)
  - `TOKEN_*` env 名保持不变(单下划线,对现有 docker-compose 零 breaking)
  - `TOKEN_DATABASE_DB_TYPE` 新增 unknown-value 兜底(warn 后保持 yaml)
  - 新增 6 个测试:`env_overlay_sets_db_type_verbatim`(regression)、numeric fields、malformed input、end-to-end
- `services/token-service/src/main.rs`
  - 移除 `common_config::Loader`
  - `CONFIG_SERVICE_URL` 兜底保留,作为**低优先级 env fallback layer**(process env 仍最优先),失败降级为无 remote(不阻塞启动)
- `services/token-service/src/lib.rs` —— pub use 更新
- `services/token-service/Cargo.toml` —— 移除 `common-config`,加 `tempfile` dev-dep

- `services/api-gateway/src/config.rs`
  - 新增 `apply_env_overrides + load_config_with_env`
  - **env 命名从双下划线改为单下划线**:`GW_SERVER__HOST` → `GW_SERVER_HOST`、`GW_REDIS__URL` → `GW_REDIS_URL`、`GW_REDIS__PREFIX` → `GW_REDIS_PREFIX` 等
  - `load_config` 从 `File::with_name` 换成 `File::from`,以支持绝对路径
  - 新增 5 个测试,含 `env_overlay_single_underscore_prefix_works` regression
- `services/api-gateway/src/main.rs` —— `load_global_config` 换用 `gw_config::load_config_with_env`

### 配置

- `infrastructure/docker-compose-services.yml`
  - api-gateway 段:`GW_SERVER__HOST/PORT/GW_REDIS__URL/PREFIX` → 单下划线
  - 过时的"由 common_config::Loader 加载"注释改为"由 config.rs::apply_env_overrides 手写映射"(token + gateway 两段都改)

### 文档

- `services/api-gateway/{README.md, AGENTS.md}`:env 表统一单下划线,追加历史注记块解释为何变
- `services/iam-service/AGENTS.md`:交叉引用 `GW_REDIS__PREFIX` → `GW_REDIS_PREFIX`
- `docs-site/guides/{api-gateway-env.md, token-service-env.md}`:重写"启动流程"段,追加"2026-07 变更"节
- `test-case/44-permission-epoch.md`:检查步骤里的 env 名同步

未改:

- `docs/superpowers/plans/*` —— 历史 spec 快照保留原样
- `release/dist-template/release/coder-micro-services-1.0.{0,1}/*` —— 是打包快照,下次 release.sh 会自动同步

## 影响

### Breaking

**api-gateway 的 4 个 env 名变化**:

| 旧 | 新 |
|---|---|
| `GW_SERVER__HOST` | `GW_SERVER_HOST` |
| `GW_SERVER__PORT` | `GW_SERVER_PORT` |
| `GW_REDIS__URL` | `GW_REDIS_URL` |
| `GW_REDIS__PREFIX` | `GW_REDIS_PREFIX` |

其他 `GW_*` env 一直是单下划线,无变化。docker-compose 已同步,dev 场景不用手动改。

**运维**:若线上环境有独立于 compose 的 env 注入(k8s ConfigMap、systemd EnvironmentFile 等),必须一并改。

### 非 breaking

token-service 全部 `TOKEN_*` env 名保持不变,只有实现路径变了。原来 compose 里的 `TOKEN_DATABASE_DB_TYPE` 现在真正生效(此前被静默丢弃)。

## 验证

- `cargo test -p token-service --lib` —— **20 passed**
- `cargo test -p api-gateway --lib` —— **25 passed**
- 全仓 `grep -r 'GW_.*__[A-Z]'` —— 零实际代码残留,只剩 4 处历史指针注释

## 未来工作

- `libs/common-config` 目前只被少数场景引用(watch_config / HotConfig / config-service provider),可以逐步收敛为纯工具函数库(去除"自动 env 绑定"这条对外 API),避免下次误用。放在下一个 phase 处理。
- 增加 `.trae/rules/project_rules.md` 约定:"config 层禁用 `config::Environment` 自动绑定,统一走 apply_env_overrides"。防止新服务再踩坑。
