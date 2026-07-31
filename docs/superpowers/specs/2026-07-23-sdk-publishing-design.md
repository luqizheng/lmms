# SDK 发布策略：kellnr + Reposilite（追认）

> 日期：2026-07-23
> 状态：追认（7 月 SDK 发布链路已上线，本 spec 补文档）
> 关联：[ADR-004 Release Engineering](../../../docs-site/decisions/ADR-004-release-engineering-architecture.md)
> 范围：`sdk/rust/*`（发布到私有 kellnr）+ `sdk/java/*`（发布到 Reposilite）+ `libs/*`（发布到 kellnr）+ `scripts/publish-maven-*.js` + `sdk/scripts/publish-sdks.sh`
> 风险：低（追认已上线链路 + 补文档）

---

## 1. 现状

### 1.1 已交付能力

7 月完成 SDK 多语言发布链路：

| SDK | 目标仓库 | 版本 | 触发脚本 |
|-----|---------|------|---------|
| Rust libs（`libs/`）| 私有 kellnr | workspace 版本 | `pnpm run sdk:publish:rust` |
| Rust SDK（`sdk/rust/*`）| 私有 kellnr | workspace 版本 | 同上 |
| Java SDK（`sdk/java/*`）| Reposilite `192.168.2.12:18181` | `0.3.0`（独立版本）| `scripts/publish-maven-release.js` / `scripts/publish-maven-snapshot.js` |
| .NET SDK | ❌ **已废弃**（7 月清理，commit `a147709`）| — | — |

### 1.2 Rust 发布链路

```bash
# 前置：setup-wizard 已配 kellnr registry + token 到 .cargo/{config,credentials}.toml
# 触发（cargo publish 每个 crate）
pnpm run sdk:publish:rust
# → 依赖顺序 publish libs/digua_common-* / libs/jwt-ctx / libs/digua_authz / sdk/rust/*
```

**发布顺序**：libs → sdk（因为 sdk 依赖 libs），单个 crate publish 后需等 kellnr 索引更新（10-30s）才能 publish 下一个。

**版本策略**：workspace 统一版本（当前 `0.1.0`），随平台整体发版。

### 1.3 Java 发布链路

```bash
# 前置：setup-wizard 已配 Reposilite servers（token 或 userpass）到 .mvn/settings.xml
# Release（tag 触发）
node scripts/publish-maven-release.js

# Snapshot（feature branch）
node scripts/publish-maven-snapshot.js

# 或统一入口
bash sdk/scripts/publish-sdks.sh
```

**发布单元**：`sdk/java/pom.xml` 是多模块 reactor：
- `audit-service-sdk` / `config-service-sdk` / `file-service-sdk` / `iam-service-sdk`
- `notification-service-sdk` / `token-server-client-sdk`
- `digua_common` / `kudi-http-client`

**版本策略**：`0.3.0` 起独立版本号，与平台版本解耦（消费方按需锁定 SDK 版本）。

### 1.4 为什么私有仓库

**放弃"发到公网 crates.io / Maven Central"的理由**：
- SDK 是内部平台工具，非开源产品
- 版本迭代速度快，公网发布合规成本高
- 私有仓库支持团队/客户内网访问
- 私有仓库无发布配额限制

---

## 2. 设计

### 2.1 Rust workspace 发布拓扑

```
libs/digua_common-error       ← 无依赖
libs/digua_common-config      ← 无依赖
libs/digua_common-log         ← 无依赖
libs/digua_common-crypto      ← 无依赖
libs/digua_common-db          ← digua_common-error
libs/digua_common-jwt         ← digua_common-error
libs/digua_common-http        ← digua_common-error
libs/digua_common-event       ← digua_common-error
libs/jwt-ctx            ← digua_common-jwt
libs/digua_authz              ← jwt-ctx
sdk/rust/*              ← 上面全部
```

发布顺序按依赖拓扑排序，`sdk/scripts/publish-rust.js` 处理拓扑排序 + kellnr 索引等待。

### 2.2 Java multi-module 发布

`sdk/java/pom.xml` 是 aggregator，`mvn deploy` 按 reactor 顺序发布所有子模块。

关键点：
- **`-pl` 参数是相对 reactor 根**（fix commit `... -pl paths must be relative to reactor root`）
- **skip tests in docker build**：Dockerfile.builder 里 `-DskipTests` 避免 docker 环境跑集成测试
- **自动 timestamps**：release/snapshot 加时间戳后缀

### 2.3 SDK 消费方接入

**Rust 消费方**（业务微服务）在 `Cargo.toml`：

```toml
[dependencies]
audit-service-sdk = { registry = "kellnr", version = "0.1.0" }
```

前置：消费方 `.cargo/config.toml` 已注册 `kellnr` registry（setup-wizard 会写入）。

**Java 消费方**在 `pom.xml`：

```xml
<dependency>
    <groupId>com.coder</groupId>
    <artifactId>audit-service-sdk</artifactId>
    <version>0.3.0</version>
</dependency>
```

前置：消费方 `.mvn/settings.xml` 已配 Reposilite mirror（setup-wizard 会写入）。

### 2.4 Docker registry 与 SDK registry 的关系

**互不影响**：
- Docker registry 是**镜像**分发（平台服务 image）
- kellnr / Reposilite 是**代码依赖**分发（业务方消费 SDK 时用）

两条链路独立，各自 setup-wizard 配置。

---

## 3. 测试计划

- [x] 已验证：`sdk:publish:rust` 拓扑排序发布 libs + sdk/rust/*
- [x] 已验证：`publish-maven-release.js` 发到 Reposilite 0.3.0
- [x] 已验证：客户端消费 Rust SDK `[dependencies] audit-service-sdk = { registry = "kellnr" }`
- [x] 已验证：客户端消费 Java SDK Maven 依赖
- [ ] 待做：SDK 版本兼容性矩阵测试（旧 SDK vs 新平台）
- [ ] 待做：SDK breaking change 前置校验（cargo semver-checks 集成）

---

## 4. 回滚方案

### 4.1 回滚触发条件

- 发布了破坏性变更导致消费方编译失败
- 发布了含 secret / 敏感数据的版本
- SDK 逻辑 bug 影响多个消费方

### 4.2 回滚步骤

**Rust（kellnr）**：
1. kellnr 支持 `yank`（`cargo yank --version <ver> --registry kellnr`），标记版本"已 yank"
2. 消费方 `cargo update` 会跳过 yanked 版本
3. **不能真删** —— kellnr 保留历史，消费方锁定的旧 `Cargo.lock` 仍能拉

**Java（Reposilite）**：
1. Reposilite 支持删除已发布 artifact（管理面板 / API）
2. 或发布 `<version>-postfix` 修正版，让消费方升级

### 4.3 回滚耗时估计

- 分钟级
- **消费方 `Cargo.lock` / `pom.xml` 锁定的版本仍能拉**——需通知消费方主动升级
- 若含 secret：yank 后应视为已泄漏，需重新签名 secret（无法真删）

### 4.4 不能回滚的情况

- **含 secret 的版本已被消费方拉取**：yank 不能撤销已下载 —— 需假设 secret 已泄漏并轮换
- **依赖此版本的消费方已上线**：yank 不影响运行时 —— 需主动通知升级

---

## 5. 已知限制 / 后续工作

- **SDK 语义化版本策略未统一**：Java 0.3.0，Rust 0.1.0，未固化 major/minor bump 规则
- **无 SDK breaking change 检测**：`cargo semver-checks` / `japicmp` 未集成 CI
- **kellnr 单点**：kellnr 挂了阻塞 Rust 发布 + 消费
- **消费方版本兼容矩阵未文档化**：SDK 0.3.0 兼容哪些平台版本？未记录
- **发布 CHANGELOG 缺失**：SDK 每个版本的变更未记录到 `sdk/CHANGELOG.md`

---

## 6. 关联

- **ADR-004** Release Engineering Architecture
- **相关 spec**: `2026-07-23-setup-wizard-design.md`（配置 kellnr / Reposilite）
- **相关 spec**: `2026-07-23-release-dist-template-design.md`（airgap 包不含 SDK，SDK 独立发布）
- **历史 spec**: `2026-06-08-multi-lang-sdk-design.md`（SDK 初始设计）
