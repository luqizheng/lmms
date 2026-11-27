# 双 Registry 发布规范

**适用仓库：** `luqizheng/lmms`  
**发布策略：** 同一个经过验证的版本，分别发布到内网和公网 Registry。  
**唯一发布触发器：** 创建符合 `v*` 的 Git tag。

## 1. 触发规则

- `main` / `develop` branch push：不发布。
- Pull Request 创建、更新、合并：不发布。
- 手动 workflow dispatch：不发布。
- 创建 `v*` tag：允许启动正式发布流水线。
- 非 `v*` tag：不发布。

发布流水线必须从 tag 对应的 commit checkout，并验证 tag 指向的 commit 已包含在 `main` 历史中。禁止移动或覆盖已存在的 tag。

## 2. 双目标发布矩阵

| 制品 | 内网目标 | 公网目标 |
|---|---|---|
| Rust crates / Rust SDK | Kellnr（`release.toml` 中配置） | crates.io 默认 registry |
| TypeScript SDK | 内部 npm Registry（`release.toml` 中配置） | npmjs.org |
| 服务镜像 | `zhcoder-docker-registry.com:8000/micro-server` | Docker Hub namespace |
| Java SDK | 内网 Reposilite | 暂不纳入公网发布，除非另行完成 Maven Central 配置 |

两个目标必须分别执行 preflight、版本冲突检查和发布；任一目标失败不得伪报双目标发布成功。内部和公网版本状态必须分别记录。

## 3. 配置隔离要求

`release.toml` 是内网目标的配置真相源，不得为了公网发布而修改或覆盖它。公网目标必须通过显式参数传入 CLI，不能依赖环境中隐式的 Cargo/npm/Docker 默认目标。

- Rust：公网 crates.io 必须使用 Cargo 默认 registry 语义（不传私有 registry alias）；内部 Kellnr 使用显式 registry alias。
- npm：每次发布都显式指定 registry URL。
- Docker：每次发布都显式指定 registry 和 namespace。
- 内网发布成功后生成的 `.image.env` 必须保留内网镜像引用；公网镜像发布不得覆盖内网部署 pin 文件。

## 4. 当前实施状态与发布阻断

截至本规范编写时，仓库 CLI 的 `release sdk` 命令只读取 `release.toml` 中的 Rust/npm registry，尚无独立的公网 registry override。Rust 发布器也明确拒绝未设置 registry 的情况。因此，当前 CLI 无法在不修改内网配置的前提下安全地向 crates.io 发布。

此外，仓库尚未通过本次审计确认内网发布凭据的 GitHub Actions Secret 名称、可用性和权限；Docker Hub namespace/repository 所有权及现存 tag 也需在执行前核验。不得猜测 Secret 名称，不得把凭据写入仓库或日志。

**因此本次先将 GitHub 发布入口收敛为 tag-only，并执行非发布型 preflight。实际制品推送保持关闭，直至完成以下工作：**

1. CLI 增加并测试 Rust/npm/Docker 的显式目标 override；Rust crates.io 路径不得错误使用 `--registry crates-io`。
2. 增加镜像发布的 pin 文件保护，公网镜像发布不能覆盖内网 `.image.env`。
3. 在 GitHub Actions 中配置并验证内网 Kellnr、npm、Docker Registry 凭据，以及公网 crates.io、npmjs.org、Docker Hub 凭据。
4. 核验 Docker Hub namespace 和目标 repositories。
5. 为双目标发布增加可重复执行/已存在版本跳过策略，并分别生成制品清单与发布报告。
6. 完成 CLI 单元测试、workflow 静态检查和 tag 预检后，才允许启用 publish jobs。

## 5. 发布报告要求

每个 tag 必须生成一份报告，至少包含：

- tag、tag commit SHA、main 基线关系；
- Rust crate、npm package、Docker image 的名称与版本；
- 内网和公网每个目标的发布结果（PUBLISHED / ALREADY_EXISTS / FAILED / NOT_ATTEMPTED）；
- 每个 registry 的验证方式和可复核证据；
- 未发布制品、失败原因及后续动作；
- 明确区分“预检通过”和“制品已发布”。

不得将 workflow 成功、构建成功或 dry-run 成功描述为制品发布成功。
