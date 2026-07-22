# Registry 来源配置

> `pnpm run setup` 时如何配置各类包源的拉取来源。
> 涵盖：Docker 镜像、Rust crates.io、Maven、npm。
> 适用于：本地开发、CI、离线分发包消费方。

---

## 概述

项目用到 4 类包源，可能来自不同 registry：

| 类别 | 包含 | 默认来源 | 可选配置 |
|------|------|----------|----------|
| **Docker 基础镜像** | rabbitmq / redis / nginx / mysql / consul / grafana 等 | Docker Hub | 指定 mirror registry |
| **Docker Builder 镜像** | rust / java / maven / node 等编译环境 | `zhcoder-docker-registry.com:8000` | 替换为其他私有 registry |
| **Rust crates.io** | Rust crate 依赖 | 官方 crates.io | 指定 mirror + token |
| **Maven** | Java 依赖 + `mvn deploy` 发布 | Maven Central | 指定 mirror + 认证（token / username+password） |
| **npm** | Node 依赖 | npmjs.com | 指定 mirror + token |

`pnpm run setup` 会通过交互式向导 `scripts/setup-wizard.js` 询问这 4 类来源，写入 `infrastructure/.docker-registry.json`（gitignored 本地配置），同时生成对应的配置文件（`.cargo/`、`.mvn/`、`.npmrc`）。

---

## 交互式向导

跑 `pnpm run setup` 时，在 `pnpm install` 后会进入向导（8 个步骤）：

```
=== Docker Registry & Cargo / Maven / Npm Setup Wizard ===

1. 基础镜像（rabbitmq / redis / nginx / mysql / consul / grafana / prometheus 等）
   默认从 Docker Hub (hub.docker.com) 拉取。若环境无法访问 Docker Hub，
   输入镜像仓库地址（如 zhcoder-docker-registry.com:8000/official）作为 mirror。

  Base registry [留空=Docker Hub]: _

2. Builder 镜像（rust / java / maven / node 等编译环境）
   默认从 zhcoder-docker-registry.com:8000 拉取。

  Builder registry [zhcoder-docker-registry.com:8000]: _

3. crates.io 镜像（Rust crate 拉取来源）
   留空 = 官方 crates.io；输入镜像 URL 作为 [source.crates-io] 的 replace-with。
   example: http://192.168.2.12:16444（自动补 sparse+ 与 /api/v1/crates/）

  crates.io mirror [留空=官方 crates.io]: _

4. crates.io / mirror 的 auth token
   留空 = 不写 token（公开镜像不需要）。

  crates.io token [留空=跳过]: _

5. Maven 镜像（Java 依赖拉取来源）
   留空 = 官方 Maven Central；输入镜像 URL 作为 <mirror> 替换 central。
   example: http://mvn.kudi.repos

  maven mirror [留空=Maven Central]: _

6. Maven 认证方式（拉取 mirror + mvn deploy 发布都用此认证）
   none     = 不写 <servers>（公开 mirror 无需认证）
   token    = 公网发布（如 Maven Central project token）
   userpass = 私有部署（reposilite / nexus 的 username + password）

  maven auth mode [none/token/userpass]: userpass
  maven username [留空=保留现有]: alice
  maven password [留空=保留现有]: _

7. npm 镜像（Node 依赖拉取来源）
   留空 = 官方 npmjs.com；输入镜像 URL 作为 registry。
   example: http://npm.kudi.repos

  npm mirror [留空=npmjs.com]: _

8. npm mirror 的 auth token
   留空 = 不写 _authToken。

  npm token [留空=跳过]: _

9. 同步到 .gitlab-ci.yml
   将配置写入 .gitlab-ci.yml 的 variables: 段（CODER_* 变量）。
   非敏感值直接写明文；敏感值引用 GitLab CI Variables。

  sync to .gitlab-ci.yml [no]: yes
```

> 所有 token / password 均为明文输入（不隐藏），但不在提示中回显已有值（避免泄露）。

---

## 各类配置的处理方式

### Docker 镜像（基础 + Builder）

**基础镜像**（如 `redis:7-alpine`）：
- 留空 = 直接从 Docker Hub 拉原始 ref
- 输入 URL = 作为 mirror 前缀，`images-pull` 先试 `<baseRegistry>/<ref>`，失败回退原始 ref

**Builder 镜像**（如 `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl`）：
- 默认 = 直接拉原始 ref
- 输入其他 registry = 重写 ref 的 registry 前缀

### Rust crates.io

- 留空 = 官方 crates.io（`.cargo/config.toml` 中不写 `replace-with`）
- 输入 URL = 写入 `.cargo/config.toml` 的 `[source.crates-io] replace-with = "coder-mirror"` + `[source.coder-mirror]`
  - 自动补 `sparse+` 前缀
  - 若只给 `host:port`（如 `http://192.168.2.12:16444`），自动补 `/api/v1/crates/`（kellnr 默认路径）
  - 也接受完整 URL（如 `sparse+https://rsproxy.cn/index/`）
- token 非空 = 写入 `.cargo/credentials.toml` 的 `[registry.coder-mirror] token = "..."`
- `.cargo/config.toml` 始终保留 kellnr 私有 registry 段 + `[net] git-fetch-with-cli = true`

### Maven

- 留空 = Maven Central（不写 `<mirrors>`）
- 输入 URL = 写入 `.mvn/settings.xml` 的 `<mirrors><mirror>` 替换 central
- 认证模式：
  - `none` = 不写 `<servers>`
  - `token` = 公网发布（`<username>token</username><password>{token}</password>`）
  - `userpass` = 私有部署（`<username>{user}</username><password>{pass}</password>`）
- `<servers>` 覆盖 3 个 id：`coder-maven-mirror`（拉取）+ `reposilite-repository-releases` + `reposilite-repository-snapshots`（`mvn deploy` 发布）
- 同时生成 `.mvn/maven.config`（`-s.mvn/settings.xml`），`mvn` 启动时自动加载
- 无 mirror 但有认证时仍生成 settings.xml（纯发布场景：Maven Central 拉取 + 私有 deploy）

### npm

- 留空 = npmjs.com（不生成 `.npmrc`）
- 输入 URL = 写入项目根 `.npmrc` 的 `registry=...`
- token 非空 = 追加 `//host/:_authToken=...`（host 从 registry URL 提取）

---

## Docker 构建时如何生效

各类配置在 `docker build` / 容器内构建时的应用方式：

| 配置 | 应用方式 | 链路 |
|------|----------|------|
| **crates.io mirror + token** | Dockerfile `COPY .cargo/` + `ENV CARGO_CREDENTIALS_PATH` | `.cargo/` 在构建上下文中 -> COPY 进镜像 -> `cargo build` 读取 |
| **Maven mirror + auth** | `build-java-sdk-docker.js` volume mount | `.mvn/settings.xml` -> mount 到容器 `/root/.m2/settings.xml` -> `mvn` 读取 |
| **npm mirror + token** | `build-docker.js` 临时复制 + Dockerfile COPY | 根 `.npmrc` -> 构建前复制到 `frontend/.npmrc` -> Dockerfile `COPY .npmrc*` -> `pnpm install` 读取 |
| **Docker base/builder registry** | `base-images-sync.js` + `images-pull.js` | `.docker-registry.json` -> 更新 `base-images.json` -> 预拉镜像 -> Dockerfile `FROM` |

关键点：
- **Rust 服务**：8 个 Dockerfile 均设 `ENV CARGO_CREDENTIALS_PATH=/app/.cargo/credentials.toml`，让 cargo 在 `docker build` 时能读取项目级 credentials（路径不存在时 cargo 自动忽略）
- **Java SDK**：`Dockerfile.builder` 是空壳镜像，实际 `mvn` 构建在 `build-java-sdk-docker.js` 的 volume mount 容器中跑
- **frontend**：构建上下文是 `frontend/`，根 `.npmrc` 不在其中，由 `build-docker.js` 构建前临时复制、构建后清理

---

## 同步到 GitLab CI

向导第 9 步询问是否将配置同步到 `.gitlab-ci.yml`。yes 时 wizard 直接编辑 `.gitlab-ci.yml` 的 `variables:` 段，写入 `CODER_*` 变量：

- **非敏感值**（registry URL / auth mode）直接写明文
- **敏感值**（token / password / username）写 `"$CI_VAR"` 引用形式，实际值需在 GitLab **Settings -> CI/CD -> Variables** 中创建

### 敏感值 -> GitLab CI Variable 映射

| Wizard 字段 | `.gitlab-ci.yml` 中的写法 | 需在 GitLab 创建的 CI Variable |
|---|---|---|
| `crateToken` | `CODER_CRATE_TOKEN: "$CRATE_TOKEN"` | `CRATE_TOKEN`（masked + protected） |
| `mavenToken` | `CODER_MAVEN_TOKEN: "$MAVEN_TOKEN"` | `MAVEN_TOKEN`（masked + protected） |
| `mavenUsername` | `CODER_MAVEN_USERNAME: "$MAVEN_USERNAME"` | `MAVEN_USERNAME`（masked + protected） |
| `mavenPassword` | `CODER_MAVEN_PASSWORD: "$MAVEN_PASSWORD"` | `MAVEN_PASSWORD`（masked + protected） |
| `npmToken` | `CODER_NPM_TOKEN: "$NPM_TOKEN"` | `NPM_TOKEN`（masked + protected） |

### 同步行为

- **幂等**：多次跑向导只更新 `CODER_*` 行，不会重复插入
- **保留格式**：不做 YAML 序列化（避免丢注释和缩进），逐行处理 `variables:` 段
- **不影响其他变量**：只增删 `CODER_*` 行，`DOCKER_REGISTRY` / `BASE_REGISTRY` / `MAVEN_REPO_URL` 等原有变量原封不动
- **空值跳过**：字段为空的不写入（如 `crateRegistry` 空 = 官方 crates.io 时不生成对应行）

### 同步后 CI 各 job 怎么用

`.gitlab-ci.yml` 的 `variables:` 段变量会作为环境变量注入所有 job。各 job 在 `before_script` 中跑 `setup-wizard --non-interactive`（如果 image 有 node），wizard 会读取这些 `CODER_*` 环境变量并生成本地配置文件（`.cargo/` / `.mvn/` / `.npmrc`），后续 `cargo build` / `mvn` / `pnpm install` 自动使用。

对于没有 node 的 image（rust / maven），可以直接在 `before_script` 中手写配置文件（`.cargo/config.toml` / `~/.m2/settings.xml`），用 `CODER_*` 环境变量填充值。

---

## 非交互模式（CI / 自动化）

stdin 不是 TTY 时自动跳过交互，按以下优先级解析配置：

**优先级**（高 -> 低）：环境变量 > 配置文件 > 默认值

| 环境变量 | 作用 | 默认值 |
|----------|------|--------|
| `CODER_BASE_REGISTRY` | Docker 基础镜像来源（空=Docker Hub） | 空 |
| `CODER_BUILDER_REGISTRY` | Docker Builder 镜像仓库 | `zhcoder-docker-registry.com:8000` |
| `CODER_CRATE_REGISTRY` | crates.io mirror URL（空=官方） | 空 |
| `CODER_CRATE_TOKEN` | crates.io / mirror auth token | 空 |
| `CODER_MAVEN_REGISTRY` | maven mirror URL（空=Maven Central） | 空 |
| `CODER_MAVEN_AUTH_MODE` | maven 认证：`none` / `token` / `userpass` | `none` |
| `CODER_MAVEN_TOKEN` | maven token（auth=token 时用） | 空 |
| `CODER_MAVEN_USERNAME` | maven username（auth=userpass 时用） | 空 |
| `CODER_MAVEN_PASSWORD` | maven password（auth=userpass 时用） | 空 |
| `CODER_NPM_REGISTRY` | npm mirror URL（空=npmjs.com） | 空 |
| `CODER_NPM_TOKEN` | npm mirror auth token | 空 |
| `CODER_SYNC_GITLAB_CI` | 同步 CODER_* 到 `.gitlab-ci.yml`（`yes` / `1` / `true`） | 关 |

> 设置 `CODER_MAVEN_TOKEN` 时自动推断 authMode=token；设置 `CODER_MAVEN_USERNAME` / `CODER_MAVEN_PASSWORD` 时自动推断 authMode=userpass。

CI 示例：

```bash
# CI 用内网 mirror 拉所有包源
CODER_BASE_REGISTRY=zhcoder-docker-registry.com:8000/official \
CODER_CRATE_REGISTRY=http://192.168.2.12:16444 \
CODER_CRATE_TOKEN=crate-secret \
CODER_MAVEN_REGISTRY=http://mvn.kudi.repos \
CODER_MAVEN_AUTH_MODE=userpass \
CODER_MAVEN_USERNAME=ci-user \
CODER_MAVEN_PASSWORD=ci-pass \
CODER_NPM_REGISTRY=http://npm.kudi.repos \
CODER_NPM_TOKEN=npm-secret \
pnpm run setup

# 或显式跳过向导（用默认值）
node scripts/setup-wizard.js --defaults
```

---

## 命令行参数

```bash
node scripts/setup-wizard.js                 # 交互式（默认，stdin 是 TTY 时）
node scripts/setup-wizard.js --non-interactive   # 跳过交互，用 env var 或配置文件
node scripts/setup-wizard.js --defaults          # 强制用默认值（全部官方源，无 token）
node scripts/setup-wizard.js --show              # 打印当前配置后退出
node scripts/setup-wizard.js --help              # 打印帮助
```

也可以用 pnpm 别名：

```bash
pnpm run setup:wizard                           # = node scripts/setup-wizard.js
pnpm run setup:wizard -- --show                 # 传参给脚本
```

---

## 配置文件

`infrastructure/.docker-registry.json`（gitignored，不入 git）：

```json
{
  "version": 4,
  "baseRegistry": "",
  "builderRegistry": "zhcoder-docker-registry.com:8000",
  "crateRegistry": "",
  "crateToken": "",
  "mavenRegistry": "",
  "mavenAuthMode": "none",
  "mavenToken": "",
  "mavenUsername": "",
  "mavenPassword": "",
  "npmRegistry": "",
  "npmToken": "",
  "updatedAt": "2026-07-22T00:00:00.000Z"
}
```

| 字段 | 含义 |
|------|------|
| `baseRegistry` | 空字符串 = Docker Hub；非空 = mirror registry URL |
| `builderRegistry` | Builder 镜像仓库，默认 `zhcoder-docker-registry.com:8000` |
| `crateRegistry` | 空字符串 = 官方 crates.io；非空 = mirror URL |
| `crateToken` | crates.io / mirror auth token |
| `mavenRegistry` | 空字符串 = Maven Central；非空 = mirror URL |
| `mavenAuthMode` | `none` / `token` / `userpass` |
| `mavenToken` | token 模式时的 token |
| `mavenUsername` | userpass 模式时的 username |
| `mavenPassword` | userpass 模式时的 password |
| `npmRegistry` | 空字符串 = npmjs.com；非空 = mirror URL |
| `npmToken` | npm mirror auth token |
| `updatedAt` | 最后更新时间（向导写入时自动填） |

文件不存在时，`base-images-sync.js` 回退到默认值（等价于 `--defaults`）。

> 旧版配置文件（version 1-3）会被自动迁移：v3 的 `mavenToken` 自动升级为 `mavenAuthMode: "token"`。

### 向导生成的其他文件

| 文件 | gitignored | 内容 |
|------|------------|------|
| `.cargo/config.toml` | 否（团队共享基线，但向导会重生成） | `[source.crates-io]` + kellnr + `[net]` |
| `.cargo/credentials.toml` | ✅ | `[registry.coder-mirror] token = "..."` |
| `.mvn/settings.xml` | ✅ | `<mirrors>` + `<servers>` |
| `.mvn/maven.config` | ✅ | `-s.mvn/settings.xml` |
| `.npmrc`（项目根） | ✅ | `registry=...` + `//host/:_authToken=...` |

> cargo 默认不读项目级 `.cargo/credentials.toml`，需设置 `CARGO_CREDENTIALS_PATH` 环境变量（向导输出有提示）。Rust 服务 Dockerfile 已内置此 ENV。

---

## 配置如何生效

`pnpm run setup` 的完整流程：

```
pnpm run setup
  ├─ pnpm install
  ├─ node scripts/setup-wizard.js    # 写入 .docker-registry.json
  │                                   #   + .cargo/config.toml + .cargo/credentials.toml
  │                                   #   + .mvn/settings.xml + .mvn/maven.config
  │                                   #   + .npmrc
  ├─ pnpm run images:sync            # 读取 .docker-registry.json，生成 base-images.json
  ├─ pnpm run images:pull            # 按 base-images.json 拉镜像
  ├─ pnpm run assets:download        # 下载 swagger-ui zip
  └─ pnpm run check:env              # 环境体检
```

Docker 构建时（`pnpm run docker:build`）：
- Rust 服务：`.cargo/` 在构建上下文中，Dockerfile COPY + `ENV CARGO_CREDENTIALS_PATH` 让 mirror + token 生效
- frontend：`build-docker.js` 构建前临时复制根 `.npmrc` 到 `frontend/`，Dockerfile `COPY .npmrc*` 拷入
- Java SDK：`build-java-sdk-docker.js` 优先 mount 项目级 `.mvn/settings.xml`

---

## 常见场景

### 场景 1：内网环境，全部走私有 mirror

```bash
$ node scripts/setup-wizard.js

  Base registry [留空=Docker Hub]: zhcoder-docker-registry.com:8000/official
  Builder registry [zhcoder-docker-registry.com:8000]:    # 回车用默认
  crates.io mirror [留空=官方 crates.io]: http://192.168.2.12:16444
  crates.io token [留空=跳过]: crate-secret
  maven mirror [留空=Maven Central]: http://mvn.kudi.repos
  maven auth mode [none/token/userpass]: userpass
  maven username [留空=保留现有]: alice
  maven password [留空=保留现有]: ********
  npm mirror [留空=npmjs.com]: http://npm.kudi.repos
  npm token [留空=跳过]: npm-secret
```

### 场景 2：只配 Docker 镜像，包源用官方

```bash
$ node scripts/setup-wizard.js

  Base registry [留空=Docker Hub]: zhcoder-docker-registry.com:8000/official
  Builder registry [zhcoder-docker-registry.com:8000]:    # 回车
  crates.io mirror [留空=官方 crates.io]:                 # 回车
  crates.io token [留空=跳过]:                             # 回车
  maven mirror [留空=Maven Central]:                      # 回车
  maven auth mode [none/token/userpass]: none
  npm mirror [留空=npmjs.com]:                             # 回车
  npm token [留空=跳过]:                                   # 回车
```

### 场景 3：公网发布 Maven（token 模式）

```bash
$ node scripts/setup-wizard.js

  maven mirror [留空=Maven Central]:                      # 回车（用 Maven Central 拉取）
  maven auth mode [none/token/userpass]: token
  maven token [留空=跳过]: central-project-token
```

效果：settings.xml 只有 `<servers>`（无 `<mirrors>`），覆盖 `reposilite-repository-releases` + `reposilite-repository-snapshots`，`mvn deploy` 用 token 认证。

### 场景 4：切回全部默认

```bash
node scripts/setup-wizard.js --defaults
```

效果：所有 registry 用官方源，无 token，删除 `.cargo/credentials.toml` / `.mvn/` / `.npmrc`。

### 场景 5：CI 自动化

```yaml
# .gitlab-ci.yml
test:
  script:
    - CODER_BASE_REGISTRY=zhcoder-docker-registry.com:8000/official \
      CODER_CRATE_REGISTRY=http://192.168.2.12:16444 \
      CODER_CRATE_TOKEN=$CRATE_TOKEN \
      pnpm run setup
    - pnpm test
```

---

## 故障排查

| 现象 | 原因 | 解决 |
|------|------|------|
| `docker pull` 超时 | 基础镜像从 Docker Hub 拉，网络不通 | 重新跑 `node scripts/setup-wizard.js`，输入内网 mirror |
| Builder 镜像拉取失败 | `zhcoder-docker-registry.com:8000` 不可达 | 重新跑向导，输入可达的 registry |
| `cargo build` 拉不到 crate | crates.io 不可达或 token 未生效 | 重新跑向导配 mirror + token；Dockerfile 已设 `CARGO_CREDENTIALS_PATH` |
| `mvn deploy` 401 | 认证未配或 server id 不匹配 | 确认 authMode + credentials 正确；`<servers>` 覆盖的 id 需与 pom.xml `<distributionManagement>` 一致 |
| `pnpm install` 拉不到包 | npm 不可达或 token 未配 | 重新跑向导配 npm mirror + token |
| `base-images.json` 里 builder ref 还是旧的 | 配置文件未更新或 `images:sync` 未重跑 | 先 `node scripts/setup-wizard.js`，再 `pnpm run images:sync` |
| CI 报 `images:sync` drift | 本地配置和 CI 不一致 | CI 用 `CODER_*` 环境变量对齐 |
| 想看当前配置 | - | `node scripts/setup-wizard.js --show` |
| 向导不出现，直接跳过 | stdin 不是 TTY（CI 环境） | 正常行为，用 env var 或 `--defaults` |
| `.docker-registry.json` 被误提交 | - | 已在 `.gitignore` 忽略；若已提交，`git rm --cached infrastructure/.docker-registry.json` |
| `.cargo/credentials.toml` 被误提交 | - | 已在 `.gitignore` 忽略 |
| `.mvn/settings.xml` 被误提交 | - | 已在 `.gitignore` 忽略 |
| `.npmrc` 被误提交 | - | 已在 `.gitignore` 忽略（只忽略项目根，不影响 `tests/e2e/.npmrc`） |

---

## 相关文件

| 文件 | 作用 |
|------|------|
| `scripts/setup-wizard.js` | 交互式向导，写入配置文件 + 生成 .cargo / .mvn / .npmrc |
| `scripts/base-images-sync.js` | 读取配置，生成 `base-images.json` |
| `scripts/lib/Base-Images-Writer.js` | `mergeMirror` 逻辑（builder ref 重写 + base mirror） |
| `scripts/images-pull.js` | 按 `base-images.json` 拉镜像（消费 mirror 字段） |
| `scripts/build-docker.js` | Docker 构建；frontend 构建前临时复制 `.npmrc` |
| `scripts/build-java-sdk-docker.js` | Java SDK 容器内构建；优先 mount `.mvn/settings.xml` |
| `infrastructure/.docker-registry.json` | 本地配置文件（gitignored） |
| `infrastructure/base-images.json` | 生成的镜像清单（入 git） |
| `.cargo/config.toml` | Rust crates.io 源配置（向导重生成，保留 kellnr + net） |
| `.cargo/credentials.toml` | crates.io mirror token（gitignored） |
| `.mvn/settings.xml` | Maven mirror + deploy 认证（gitignored） |
| `.mvn/maven.config` | Maven 自动加载 settings.xml 的参数（gitignored） |
| `.npmrc` | npm mirror + token（gitignored，项目根） |

---

## 相关文档

- [新人入门 § 02 环境准备](../onboarding/02-环境准备)
- [用户手册 § 01 快速上手](../user-manual/01-quick-start)
- [用户手册 § 12 发布与分发](../user-manual/12-release-and-distribute)
- [Rust SDK 发布指南](./rust-sdk-publishing)
- [Java SDK 发布指南](./java-sdk-publishing)
