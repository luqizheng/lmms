# ADR-007: 项目重命名为 "Digua"（出发吧 地瓜）

> 日期：2026-07-31
> 状态：已采纳
> 触发：项目计划开源到公共平台（GitHub + crates.io + Maven Central + npmjs），原内部品牌名不再适合公开生态。

## 上下文

`low-memory-micro-services` 项目历史上以内部品牌命名包：

| 维度 | 当前命名 | 问题（开源后） |
|------|----------|---------------|
| Rust crates（17 + 7） | `common-jwt`, `iam-service-sdk`, `kudi-http-client` 等 | `kudi-` 是内部公司品牌，公共 registry 上不合适 |
| Maven groupId | `com.kudi.coder` | 同样含内部品牌 `kudi.coder` |
| Maven artifactId | `kudi-coder-sdk-common`, `iam-service-sdk` 等 | 与新品牌不一致 |
| npm scope | `@coder-micro-services/dist-tools` | 仓库级 scope 不适合作为独立公共包 |
| 仓库名 | `coder-micro-services` / `low-memory-micro-services` | 内部代号 |

`kudi` / `coder-micro-services` 在内部使用没问题，但放到公共 registry（crates.io / Maven Central / npmjs）会：

1. **品牌混淆**：用户看到 `kudi-` 前缀会以为是某个公司的私有产品，影响采用意愿
2. **搜索不利**：`kudi` 没有语义价值（不像 `serde`、`tokio` 这种能讲出故事的名字）
3. **改名成本递增**：越晚改，引用越多越难动

## 决议

新代号：**Digua**（中文"地瓜"，项目代号"出发吧 地瓜"）

**全平台命名空间**：

| 平台 | 当前 | 改后 | 示例 |
|------|------|------|------|
| Maven Central groupId | `com.kudi.coder` | `io.digua` | `io.digua:digua-common-jwt` |
| Maven parent artifactId | `kudi-coder-sdk-parent` | `digua-sdk-parent` | — |
| crates.io (libs/) | `common`, `common-cache`, ... | `digua-common`, `digua-common-cache`, ... | `digua-common-jwt` |
| crates.io (sdk/rust/) | `iam-service-sdk`, `kudi-http-client`, ... | `digua-iam-sdk`, `digua-http-client`, ... | `digua-iam-sdk` |
| npm scope | `@coder-micro-services/...` | `@digua/...` | `@digua/dist-tools` |
| CLI 命令 | `cms-sync` | **保留不变**（命令面已面向用户，不破坏兼容性） |

**保留不变（避免破坏现有部署）**：

- **仓库名**：`coder-micro-services` / `low-memory-micro-services` 不动
- **文档品牌名**：项目显示名仍为 "Coder Micro Services" 或 "出发吧 地瓜"
- **业务服务名**：`iam-service` / `token-service` / `audit-service` 等保持（Consul 注册、env、CI 都引用这些）
- **JWT audience**：`"coder-micro-services"` 保持（已签发的 token 不能失效）
- **Docker 镜像名**：`micro-server/*` 保持（用户范围选了"只改 3 个语言包名"）
- **Cargo workspace member 路径**：`libs/*` `sdk/rust/*` 等（filesystem 路径不动，仅 crate 名加前缀）

**版本起点**：下一个开源版本作为 **`1.0.0`**（首个公共 release），不沿用 `0.x.x` 序列。

**硬切 vs 双坐标**：**硬切**。Maven Central 上无法修改已发布 artifact 的 groupId，硬切最干净——避免同时维护两套坐标带来的文档/示例混乱。已在私有 registry 发布的版本不公开即可，下游项目需要在升级到 1.0.0 时一次性改坐标。

### 完整重命名映射表

#### Rust crates（libs/）

| 当前 | 改后 |
|------|------|
| `authz` | `digua-authz` |
| `common` | `digua-common` |
| `common-cache` | `digua-common-cache` |
| `common-config` | `digua-common-config` |
| `common-crypto` | `digua-common-crypto` |
| `common-db` | `digua-common-db` |
| `common-error` | `digua-common-error` |
| `common-event` | `digua-common-event` |
| `common-health` | `digua-common-health` |
| `common-http` | `digua-common-http` |
| `common-jwt` | `digua-common-jwt` |
| `common-log` | `digua-common-log` |
| `common-ratelimit` | `digua-common-ratelimit` |
| `common-telemetry` | `digua-common-telemetry` |
| `common-telemetry-axum` | `digua-common-telemetry-axum` |
| `common-validator` | `digua-common-validator` |
| `jwt-ctx` | `digua-jwt-ctx` |

#### Rust SDK crates（sdk/rust/）

| 当前 | 改后 |
|------|------|
| `audit-service-sdk` | `digua-audit-sdk` |
| `config-service-sdk` | `digua-config-sdk` |
| `file-service-sdk` | `digua-file-sdk` |
| `iam-service-sdk` | `digua-iam-sdk` |
| `notification-service-sdk` | `digua-notification-sdk` |
| `token-service-sdk` | `digua-token-sdk` |
| `kudi-http-client` | `digua-http-client` |

#### Maven（sdk/java/ + examples/）

| 当前 groupId | 改后 groupId |
|--------------|---------------|
| `com.kudi.coder` | `io.digua` |
| `com.kudi.coder.demo` | `io.digua.demo` |

| 当前 artifactId | 改后 artifactId |
|-----------------|-----------------|
| `kudi-coder-sdk-parent` | `digua-sdk-parent` |
| `kudi-coder-sdk-common` | `digua-sdk-common` |
| `kudi-http-client` | `digua-http-client` |
| `notification-service-sdk` | `digua-notification-sdk` |
| `token-server-client-sdk` | `digua-token-client-sdk` |
| `token-server-user-sdk` | `digua-token-user-sdk` |
| `user-service-client-sdk` | `digua-user-client-sdk` |
| `user-service-user-sdk` | `digua-user-user-sdk` |

> `token-server-*` / `user-service-*` 同步改名以保持前缀统一（避免 `token-server-X` 但 `digua-X` 这种半旧半新）。

#### NPM（release/dist-tools/）

| 当前 | 改后 |
|------|------|
| `@coder-micro-services/dist-tools` | `@digua/dist-tools` |
| `cms-sync`（CLI 命令） | `cms-sync`（**保留**，避免破坏已发布下游使用方式） |

> CLI 命令 `cms-sync` 不改是因为命令面已被大量文档固化（README / docs-site / CI 脚本），改了反而制造额外迁移成本。包名 `digua` + 命令 `cms-sync` 的组合不矛盾：scope 是发布命名空间，命令是用户体验层。

## 后果

### 立即收益

1. **公共 registry 友好**：`io.digua:digua-common-jwt` 看起来像独立的公共 Rust 框架而非某个公司的产品
2. **品牌一致性**：所有平台共享同一品牌前缀 `digua`
3. **可发现性**：`digua` 在 Maven Central / crates.io / npm 上均无占用，新用户搜索时不会被内部品牌词误导

### 已知影响

1. **breaking change**：所有下游项目需要一次性把 `Cargo.toml` / `pom.xml` / `package.json` 里的依赖坐标全替换
2. **私有 registry 历史**：已经发布到 `kellnr` / 私有 Maven / 私有 npm 的 0.x 版本会被新发布的 1.0.0 取代；旧坐标不可用但代码可以通过 git tag 找回
3. **文档大扫除**：`docs-site/` 中提到旧名的位置需要更新（`coder-micro-services` 作为项目显示名保留；`com.kudi.coder` / `kudi-coder-sdk-*` / `@coder-micro-services/*` 这些**包坐标**必须更新；JWT audience 字符串保留）
4. **CI 配置**：`scripts/publish-*.js` 中 Maven groupId 写死需改；kellnr / private Maven registry 配置不动（继续发布新坐标即可）

### 不在本次范围（用户明确选择）

- 仓库 GitHub / GitLab 名重命名
- Docker 镜像 `micro-server/*` → `digua/*`
- 文档站品牌名 "Coder Micro Services" → "Digua"
- 业务服务名（iam-service 等）

这些都可以在未来的 ADR 里单独决策。

## 实施步骤

1. 新建任务分支 `feat/digua-rename`
2. Rust crates 改名（17 + 7）+ 更新所有 path 依赖
3. Maven groupId / artifactId 改名（sdk/java + examples）+ 更新所有 `<dependency>` 引用
4. npm dist-tools 改名 + 重新发布为 `@digua/dist-tools@1.0.0`
5. docs-site 中包坐标引用更新（`@coder-micro-services/dist-tools` → `@digua/dist-tools`）
6. 跑 `cargo check --workspace` / `mvn validate` / `pnpm test` 验证
7. 发布 v1.0.0 到私有 registry（私有发布验证流程）
8. 准备公开发布 checklist（Maven Central 账号注册 / crates.io ownership 转移 / npm public registry 切换）