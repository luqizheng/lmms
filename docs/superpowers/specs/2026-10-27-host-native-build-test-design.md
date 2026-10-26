# 主机侧编译与测试（把 cargo 从 Docker 搬到主机）

> 日期：2026-10-27
> 关联：`.trae/rules/project_rules.md` §1.3（改代码前先有 spec）、AGENTS.md「开发环境」
> 范围：`scripts/**`、`package.json`（不改任何 `services/**` / `libs/**` 业务代码）
> 风险：低（只增加一条开发期工作流；Docker 路径原样保留）

---

## 1. 现状

### 1.1 具体行为

仓库的 Rust 编译与测试目前全部在 Docker 内完成：

- 测试：`node scripts/run-tests-docker.js`（= `pnpm run test:rust`），基础镜像
  `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl`，target `x86_64-unknown-linux-musl`，
  源码 bind mount，`registry` / `git` / `target` 用命名 volume 复用增量编译。
- 发布镜像：`node scripts/build-docker.js`（= `pnpm run docker:build`）。

本机（Windows 11 + Docker Desktop 29.8.2）上，容器内编译要付的代价是：改一行 Rust → 进容器 →
musl 目标重新链接 → 产物只在 volume 里看不到；本机也没有可复用的增量缓存。

### 1.2 根因（为什么不能简单地在 Windows 上 `cargo test`）

本机实测（2026-10-27），Windows 原生 MSVC 工具链走不通，阻塞点在 C 依赖，不在仓库代码：

```
# 有 MSVC 开发环境时：
warning: ring@0.17.7: cl: fatal error D8050 : 无法加载 ...\VC\Tools\MSVC\14.51.36231\bin\HostX64\x64\c1.dll
# 没有 MSVC 开发环境时：
fatal error C1083: 无法打开包括文件: “stddef.h”
```

`ring 0.17.7` 与 `aws-lc-sys 0.41.0` 都要用 `cl.exe` 编译 C 代码；`cl.exe` 需要 `c1.dll`
与 MSVC/CRT 头文件，两者只有在 VS 开发环境（`vcvars64.bat`）里才稳定可用，而 cargo 的
build script 子进程拿不到这套环境。同一份源码在 WSL2（原生 gcc）里编译完全正常。

### 1.3 影响面

只影响开发迭代速度，不影响生产：CI 门禁（`scripts/run-tests-docker.js`）与发布镜像构建
仍走 Docker，行为不变。

---

## 2. 设计方案

### 2.1 方案选择

| 候选 | 结论 |
|------|------|
| A. Windows 原生 MSVC 编译 | **否决**：C 依赖（ring / aws-lc-sys）在本机 cl.exe 下不可用（见 1.2）。 |
| B. WSL2 发行版里编译测试（镜像网络直通数据库） | **选中**：工具链与构建镜像同代（Rust 1.96 + gcc），原生 Linux 语义，target 放 WSL 原生盘后增量编译很快。 |
| C. 继续全 Docker | 保留为 CI / 发布路径，不作为日常开发路径。 |

方案 B 的关键前提（已实测）：WSL 处于镜像网络模式，能直连 Windows 侧 Docker 暴露的端口
（`127.0.0.1:33306` MySQL 8、`127.0.0.1:36379` Redis），因此**不需要**在 WSL 里再装一套数据库，
共享测试容器照旧复用。

### 2.2 改动清单

| 文件 | 改动 | 原因 |
|------|------|------|
| `scripts/lib/host-runner.js` | 新增 | 执行后端抽象：Windows 默认 WSL2，Linux/macOS 直接本机；负责路径转换、env 注入、target 目录选址 |
| `scripts/lib/services.js` | 新增 | service 清单 + 「git 改动 → service」增量判定，docker 与 host 两个脚本共用一份 |
| `scripts/lib/shared-db.js` | 新增 | 共享 MySQL/Redis 探活；不通时只 `docker start` 已有容器，不创建、不删除 |
| `scripts/run-tests-host.js` | 新增 | 主机侧测试入口：两阶段（先编译后跑），输出与 docker 版一致的 summary |
| `scripts/build-cargo.js` | 改造 | 原本用目录名当 cargo package 名（`-p api-gateway` 必然失败），也未注入 swagger vendor env；改为走主机侧后端 |
| `scripts/run-tests-docker.js` | 改造 | service 清单与增量判定改为引用 `scripts/lib/services.js`（纯提取，行为不变） |
| `package.json` | +1 行 | 新增 `test:rust:host` |

### 2.3 调用路径

```
pnpm run test:rust:host [services...] [--incremental] [--no-db] [--backend wsl|native]
   │
   ├─ resolveRunner()                    scripts/lib/host-runner.js
   │     ├─ backend=wsl    → wsl.exe -d <distro> -- bash -lc "cd /mnt/d/... && export ... && cargo ..."
   │     └─ backend=native → cargo ...
   │
   ├─ ensureMysql/ensureRedis            scripts/lib/shared-db.js
   │     └─ 探活 127.0.0.1:33306 / :36379 → 不通则 docker start coder-test-*
   │
   ├─ phase 1: cargo test --no-run -p <pkg>   （编译，失败的 service 跳过 phase 2）
   └─ phase 2: cargo test -p <pkg>            （有 cargo-nextest 时改用 nextest）
```

注入给 cargo 的 env：

| 变量 | 值 | 说明 |
|------|-----|------|
| `SWAGGER_UI_DOWNLOAD_URL` | `file://<vendor zip>` | 让 `utoipa-swagger-ui` 的 build.rs 不去连 GitHub |
| `TEST_MYSQL_HOST` / `TEST_MYSQL_PORT` | `127.0.0.1` / `33306` | 共享 MySQL 容器；`--no-db` 时不注入 |
| `DATABASE_URL` | `mysql://root:@127.0.0.1:33306/superpowers` | 部分服务 config 加载必填 |
| `REDIS_URL` / `REDIS_PREFIX` | `redis://127.0.0.1:36379` / `superpowers:` | 红黑名单 / perm_epoch 等集成测试需要 |
| `CONSUL_ADDR` | `http://localhost:8500` | 服务发现契约字段 |
| `CARGO_TARGET_DIR` | `~/.cache/cargo-target/low-memory-micro-services` | 放 WSL 原生盘；忽略 Windows 侧的 Windows 路径取值 |

---

## 3. 测试计划

- [x] 主机侧编译：`node scripts/build-cargo.js config-service --debug` 通过
- [x] 主机侧集成测试：`node scripts/run-tests-host.js config-service` 通过（连共享 MySQL）
- [x] 增量模式：`node scripts/run-tests-host.js --incremental` 正确收敛到改动涉及的 service
- [x] Docker 路径未回归：`node scripts/run-tests-docker.js --help` / service 选择参数行为不变
- [ ] CI 门禁复跑（合并前）：`node scripts/run-tests-docker.js`（全量）

---

## 4. 回滚方案

### 4.1 回滚触发条件

- 主机侧路径出现与 docker 路径不一致的测试结果（同一 commit 在两条路径结论不同）。
- `scripts/lib/services.js` 的提取改变了 `run-tests-docker.js` 的 service 选择行为。

### 4.2 回滚步骤

1. 删除新增文件：`scripts/run-tests-host.js`、`scripts/lib/host-runner.js`、
   `scripts/lib/services.js`、`scripts/lib/shared-db.js`。
2. `git checkout HEAD~1 -- scripts/run-tests-docker.js scripts/build-cargo.js package.json`。
3. CI 门禁仍走 `pnpm run test:rust`（Docker 路径），无需额外操作。

### 4.3 回滚耗时估计 + 数据损失面

分钟级；无 schema / 数据面改动，无消息或状态污染。

### 4.4 不能回滚的情况

无。本改动只影响本地开发工作流。

---

## 5. 已知限制 / 后续工作

- Windows 原生 MSVC 后端（`--backend native`）在本机不可用：C 依赖的 `cl.exe` 加载问题
  （D8050）未解决，该分支仅在 Linux/macOS 或修好 MSVC 环境的机器上可用。
- 主机侧编译产物是 `x86_64-unknown-linux-gnu`，与发布镜像的 musl 产物不同；发布镜像仍必须
  走 `build-docker.js`。
- 上游 `scripts/` 变更会随 cms-sync 穿透到下游 `project-manager`，下游无需改动即可继续用
  原有 Docker 路径；要用主机路径需自行确认 WSL 环境。

---

## 6. 关联 ADR / spec

- 既有约定：`AGENTS.md`「开发环境」、`.trae/rules/wsl-development-rules.md`（Windows 推荐 WSL）
- 无新增 ADR；本 spec 记录的是工作流决策。
