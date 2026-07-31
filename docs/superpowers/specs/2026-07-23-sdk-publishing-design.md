# SDK 发布策略：kellnr + Reposilite（追认）

> 日期：2026-07-23（初版）；2026-07-31（追认 VERSION 必填 + publish-sdks.sh 删除）
> 状态：追认（7 月 SDK 发布链路已上线，本 spec 补文档）
> 关联：[ADR-004 Release Engineering](../../../docs-site/decisions/ADR-004-release-engineering-architecture.md)
> 范围：`sdk/rust/*`（发布到私有 kellnr）+ `sdk/java/*`（发布到 Reposilite）+ `libs/*`（发布到 kellnr）+ `scripts/publish-maven-*.js` + `libs/scripts/publish-libs.sh` + `sdk/scripts/publish-rust-sdks.sh`
> 风险：低（追认已上线链路 + 补文档）

---

## 1. 现状

### 1.1 已交付能力

7 月完成 SDK 多语言发布链路：

| SDK | 目标仓库 | 版本 | 触发脚本 |
|-----|---------|------|---------|
| Rust libs（`libs/`）| 私有 kellnr | 必填位置参数 `<VER>` | `bash libs/scripts/publish-libs.sh <VER>` |
| Rust SDK（`sdk/rust/*`）| 私有 kellnr | 必填位置参数 `<VER>`（与 libs 同号）| `bash sdk/scripts/publish-rust-sdks.sh <VER>` |
| Java SDK（`sdk/java/*`）| Reposilite `192.168.2.12:18181` | 必填 CLI 参数 `--version <VER>`（脚本自动 bump `sdk/java/pom.xml` 的 `<version>`） | `node scripts/publish-maven-release.js --version <VER>` / `--snapshot.js --version <VER>-SNAPSHOT` |
| .NET SDK | ❌ **已废弃**（7 月清理，commit `a147709`）| — | — |

等价 pnpm alias：`pnpm sdk:publish:libs -- <VER>` / `pnpm sdk:publish:rust -- <VER>` / `pnpm sdk:publish:release -- --version <VER>` / `pnpm sdk:publish:snapshot -- --version <VER>`。

### 1.2 Rust 发布链路

```bash
# 前置：setup-wizard 已配 kellnr registry + token 到 .cargo/{config,credentials}.toml
# 触发（cargo publish 每个 crate；VERSION 必填）
bash libs/scripts/publish-libs.sh 0.2.0
bash sdk/scripts/publish-rust-sdks.sh 0.2.0
# → 依赖顺序 publish libs/digua_common-* / libs/jwt-ctx / libs/digua_authz / sdk/rust/*
```

**发布顺序**：libs → sdk（因为 sdk 依赖 libs），单个 crate publish 后需等 kellnr 索引更新（10-30s）才能 publish 下一个。

**版本策略**：libs 与 sdk 共用同一版本号（用户必填位置参数 `<VER>`），随平台整体发版。

### 1.3 Java 发布链路

```bash
# 前置：setup-wizard 已配 Reposilite servers（token 或 userpass）到 .mvn/settings.xml
# Release（tag 触发；--version 必填）
node scripts/publish-maven-release.js --version 0.4.0

# Snapshot（feature branch；--version 必须带 -SNAPSHOT 后缀）
node scripts/publish-maven-snapshot.js --version 0.4.1-SNAPSHOT
```

**发布单元**：`sdk/java/pom.xml` 是多模块 reactor：
- `audit-service-sdk` / `config-service-sdk` / `file-service-sdk` / `iam-service-sdk`
- `notification-service-sdk` / `token-server-client-sdk`
- `digua_common` / `digua-http-client`

**版本策略**：必填 `--version <VER>`。脚本在 deploy 前自动跑 `mvn versions:set -DnewVersion=<VER> versions:commit` 把父 pom 与子模块对齐。**pom.xml 改动不自动 git commit**，需手动 `git diff sdk/java/ && git commit -m "feat(sdk): bump to <VER>"`。

> **历史**：`scripts/publish-sdks.sh`（旧 4 模块脚本，硬编码 `SDK_VERSION=0.3.0`）已于 2026-07-31 删除，被 `publish-maven-{release,snapshot}.js` 完全替代。

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

发布顺序按依赖拓扑排序，`libs/scripts/publish-libs.sh` 与 `sdk/scripts/publish-rust-sdks.sh` 各自处理拓扑排序 + kellnr 索引等待。**VERSION 由调用方必填**（`bash publish-libs.sh <VER>` / `bash publish-rust-sdks.sh <VER>`）。

### 2.2 Java multi-module 发布

`sdk/java/pom.xml` 是 aggregator。`scripts/publish-maven-release.js --version <VER>`（或 `-snapshot.js`）在 deploy 前自动 `mvn versions:set -DnewVersion=<VER>`，然后按 reactor 顺序发布所有子模块。

关键点：
- **`-pl` 参数是相对 reactor 根**（fix commit `... -pl paths must be relative to reactor root`）
- **skip tests in docker build**：Dockerfile.builder 里 `-DskipTests` 避免 docker 环境跑集成测试
- **自动 timestamps**：release/snapshot 加时间戳后缀
- **`--version <VER>` 必填**：脚本不接受隐式默认值，避免发错版本号

### 2.3 SDK 消费方接入

**Rust 消费方**（业务微服务）在 `Cargo.toml`：

```toml
[dependencies]
audit-service-sdk = { registry = "kellnr", version = "0.2.0" }
```

前置：消费方 `.cargo/config.toml` 已注册 `kellnr` registry（setup-wizard 会写入）。

**Java 消费方**在 `pom.xml`：

```xml
<dependency>
    <groupId>com.coder</groupId>
    <artifactId>audit-service-sdk</artifactId>
    <version>0.4.0</version>
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

- [x] 已验证：`bash libs/scripts/publish-libs.sh 0.2.0 --dry-run` 拓扑排序发布 libs（dry-run 不改 Cargo.toml）
- [x] 已验证：`bash sdk/scripts/publish-rust-sdks.sh 0.2.0 --dry-run` 拓扑排序发布 sdk/rust/*
- [x] 已验证：`node scripts/publish-maven-release.js --version 0.4.0 --dry-run` 发到 Reposilite（自动 bump pom.xml）
- [x] 已验证：客户端消费 Rust SDK `[dependencies] audit-service-sdk = { registry = "kellnr" }`
- [x] 已验证：客户端消费 Java SDK Maven 依赖
- [x] 已验证：旧 `scripts/publish-sdks.sh` 已删除，无残留引用
- [x] 已验证：`.gitlab-ci.yml` `sdk_publish` / `rust_publish` job 在 tag 触发时自动注入 `${CI_COMMIT_TAG#v}` 作为 VERSION
- [ ] 待做：SDK 版本兼容性矩阵测试（旧 SDK vs 新平台）
- [ ] 待做：SDK breaking change 前置校验（cargo semver-checks 集成）

---

## 3.1 2026-07-31 改动追认

| 项 | 旧 | 新 |
| --- | --- | --- |
| `scripts/lib/maven-publish.js` | 从 `sdk/java/pom.xml` 隐式读版本 | 增加必填 `--version <VER>` CLI 参数 + `bumpPomVersion()` 自动跑 `mvn versions:set` |
| `libs/scripts/publish-libs.sh` | `--version <VER>` 可选（默认 0.1.0） | `<VER>` 位置参数**必填**，缺参 exit 2；并修复 `--dry-run` 实际改 Cargo.toml 的副作用 bug |
| `sdk/scripts/publish-rust-sdks.sh` | `--version <VER>` 可选（默认 0.1.0） | `<VER>` 位置参数**必填**，缺参 exit 2；并修复 `BASE_CRATES` 仍指向旧 `kudi-http-client` 目录的 bug + `--dry-run` 副作用 |
| `scripts/publish-sdks.sh` | 旧 4 模块脚本（硬编码 `SDK_VERSION=0.3.0`） | **已删除**（`git rm`），文档中所有引用同步移除 |
| `package.json` | `sdk:publish:release` / `sdk:publish:snapshot` | 新增 `sdk:publish:libs` / `sdk:publish:rust` 两条 alias |
| `.gitlab-ci.yml` | `sdk_publish` 隐式从 pom.xml 读版本；`rust_publish` 不传版本 | 两个 job 都显式注入 `${CI_COMMIT_TAG#v}` 作为 VERSION |

**理由**：VERSION 隐式从源码读容易出错（CI 与本地不一致；手工 mvn versions:set 后忘记 deploy）。强制显式传参 + 自动 bump 让"目标版本"在调用点清晰可见。

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
