# v1.8.1 内部发布最终报告

**日期：** 2026-11-23
**类型：** INTERNAL RELEASE（仅内部 Registry，无任何公开发布）
**最终 main SHA：** `1dbc6a09`（发布执行期修复链末端；tag 指向 `7c382118`，见 §1）
**Tag：** `v1.8.1`（annotated，指向 `7c382118` = PR #10 合并 commit，已推送 GitHub）

---

## 1. SHA 记录

| 项 | SHA | 说明 |
| --- | --- | --- |
| PR #10 合并（发布内容进 main） | `7c382118` | 44 文件：三生态目标切内部 + 守卫 + 版本 1.8.1 |
| **tag `v1.8.1` 指向** | `7c382118` | annotated；tag 不因后续修复移动 |
| `d9dff3b1` | fix(cargo) | `[registries.kellnr]` 表缺失，`cargo publish --registry` 找不到地址 |
| `477e42a0` | fix(cli) | `rewritePathDeps` 对自带 version 的 path 依赖产生重复 version 键 |
| `c6794e36` | chore(openapi) | 版本 bump 后未重新生成 openapi.yaml（utoipa 写入 `info.version`），CI drift 检查失败 |
| `1dbc6a09` | fix(cli) | `writeImageEnv` 重写时丢掉手工维护的 `IMAGE_NGINX` 基础镜像行 |

## 2. 发布目标（全部为内部 Registry，多源确认）

| 生态 | Registry | 配置 |
| --- | --- | --- |
| Rust | Kellnr `http://192.168.2.12:16444` | `release.toml [sdk].rust_registry="kellnr"` + `.cargo/config.toml [registries.kellnr]/[source.kellnr]` |
| npm | `http://192.168.2.12:14873/` | `release.toml [sdk].npm_registry` + 7 个包 `publishConfig` |
| Docker | `zhcoder-docker-registry.com:8000/micro-server` | `release.toml [registry]`（HTTP registry，buildx buildkitd-config 声明 http=true） |

## 3. Rust crate 发布清单（Kellnr，共 23 个 @1.8.1）

**libs（17，拓扑序）**：digua_common_error, digua_common_health, digua_common_jwt, digua_common_ratelimit, digua_common_telemetry, digua_common_cache, digua_common_config, digua_common_crypto, digua_common_http, digua_common_log, digua_common_telemetry_axum, digua_common_validator, digua_jwt_ctx, digua_authz, digua_common_db, digua_common, digua_common_event
**sdk（6）**：digua_http_client, digua_audit_sdk, digua_file_sdk, digua_iam_sdk, digua_notification_sdk, digua_token_sdk
（common_error 与 libs 前 4 个在修复后重跑中断点续发：18 published + 5 skipped-already-exists）

**Registry 验证**：sparse index 抽查 5 个 crate（common_error/common_db/common/http_client/token_sdk）均含 `1.8.1` ✓

## 4. npm package 发布清单（内部 npm，共 7 个 @1.8.1）

@digua/contracts, @digua/http, @digua/iam-services, @digua/audit-services, @digua/file-services, @digua/notification-services, @digua/prue-services —— 全部 published 7 / skipped 0。
`@digua/cli`（private 0.0.0）未发布 ✓。

**Registry 验证**：7 包 `1.8.1` 均存在且 `dist-tags.latest = 1.8.1` ✓

## 5. Docker image 发布清单（内部 Registry，共 7 个，built 7 / failed 0）

| 镜像 | Tag |
| --- | --- |
| micro-server/api-gateway | 1.8.1 |
| micro-server/token-service | 1.8.1-pg |
| micro-server/iam-service | 1.8.1-pg |
| micro-server/notification-service | 1.8.1-pg |
| micro-server/file-service | 1.8.1-pg |
| micro-server/audit-service | 1.8.1-pg |
| micro-server/prue-admin-backend | 1.8.1-pg |

构建推送经 buildx `cross-builder`（docker-container driver，buildkitd-config 声明 registry http=true，参照内部文档 `docker-cross-platform`）。`.image.env` 两单元已由 CLI 自动钉版 1.8.1。

## 6. 凭据权限预检（结果；凭据本体不入仓不入报告）

| 生态 | 检查 | 结果 |
| --- | --- | --- |
| Kellnr | registry 可达 + 发布 token（用户提供，会话内使用） | ✅ 发布成功即权限证明 |
| 内部 npm | `npm whoami --registry http://192.168.2.12:14873/` | ✅ `admin` |
| 内部 Docker | `docker login`（用户在发布机配置）+ registry `/v2/` 探测 | ✅ 构建推送成功即权限证明 |

未通过发布占位包验证权限；所有检查为只读或直接发布本身。

## 7. 安装 / 拉取验证（真实）

| 生态 | 验证 | 结果 |
| --- | --- | --- |
| Rust | `cargo info digua_common_error --registry kellnr`（真实走 kellnr index 下载） | ✅ 1.8.1 |
| npm | 临时目录 `npm install @digua/contracts@1.8.1 --registry http://192.168.2.12:14873/`，产物 package.json + dist 齐全 | ✅ |
| Docker | `docker pull 192.168.2.182:8000/micro-server/api-gateway:1.8.1` 与 `token-service:1.8.1-pg` | ✅ 均成功 |

⚠ Docker **域名形式**的 `docker pull`（daemon 层）在本机仍报 EOF：daemon 对该 insecure registry 的 HTTP fallback 未生效（buildkit 层已由 buildkitd-config 解决，发布与制品不受影响）。下游若遇同样问题，参照内部文档配置 buildx，或用 IP 形式拉取（`.image.env` 保持域名形式，内网 DNS 环境下正常）。

## 8. 修改文件清单

**PR #10（`7c382118`，44 文件）**：release.toml、release-toml.js、sdk-publish.js（registry 守卫 + 禁公网回退）、.cargo/config.toml、7 个 package.json（publishConfig+1.8.1）、Rust 25 个 Cargo.toml（版本+path 依赖）、Cargo.lock、两个 .image.env、public-release.yml（移除 tag 自动触发）、package.json（release:* 入口）、CLI 测试、AGENTS.md、README.md。
**发布期修复（4 commit，直接入 main）**：.cargo/config.toml、packages/cli/lib/sdk-publish.js + 测试、services/*/openapi.yaml（5 个）、packages/cli/lib/image-env.js。

## 9. CI 结果

- **PR #10**：dialect-matrix 5 job ✅、core-crates ✅；`generate-and-check` ❌（rsproxy 网络抖动 → 后由 c6794e36 修复真实 drift）、`Validate release candidate` ❌（同 drift）。PR 在 checks 未全绿状态下被合并（用户决定），失败根因已在 main 修复。
- **main 后续 push（最终 HEAD `116769f4`，含本报告）**：core-crates ✅ / openapi ✅（c6794e36 修复 drift 后恢复）/ dialect-matrix ✅（首跑 `test-services-postgres` 因 rsproxy 上游网关 504 下载超时失败，`gh run rerun --failed` 后全 job success）。
- **发布不在 CI 执行**（PR #10 起 tag 推送不触发任何自动发布），全部在本机脚本完成。

## 10. 未解决问题

1. Docker daemon 层对 `zhcoder-docker-registry.com:8000` 的 HTTP fallback 不生效（见 §7 ⚠）——影响本机 `docker pull` 域名形式；buildkit/push 路径已通过 buildkitd-config 解决。待 Docker Desktop 完整重启后复测。
2. 内部 npm 上 `@digua/contracts`/`@digua/http` 存在历史线 `1.10.1`（本次发布 `1.8.1` 且 latest 正确指向 1.8.1）；`1.10.x` 来源与本仓 package.json 版本线（0.1.0 → 1.8.1）不一致，建议后续厘清该历史线的发布来源。
3. `buildx cross-builder` 依赖 `C:\docker-config\docker.toml`（本机文件）与 buildkit 容器内的 hosts 注入（容器重建后需重新注入 `192.168.2.182 zhcoder-docker-registry.com`）；换机发布需按内部文档重建。
4. 内网 Docker Registry 为纯 HTTP：BuildKit/daemon 均需 insecure 声明；生产建议升级 TLS。

---

## 最终标记

- **INTERNAL RELEASE：SUCCESS**
- **PUBLIC RELEASE：NOT PERFORMED**
