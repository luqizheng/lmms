# Plan：新增 `coder-dev-bootstrap` Skill（首次启动开发环境）

## Summary

新增 Skill：`coder-dev-bootstrap`，用于「数据库表已清空 / 拉取了最新源码 / 第一次启动开发环境」场景，一键引导执行：环境前置检查 → 同步基础镜像 → 启动 infrastructure → 等待健康 → 服务自迁移建表 → 状态核查 → 可选 e2e 验证。

落地形式：在 `.trae/skills/coder-dev-bootstrap/SKILL.md` 创建一份 SKILL，沿用现有同类 Skill（[coder-e2e-test/SKILL.md](file:///d:/projects/coder-micro-services/.trae/skills/coder-e2e-test/SKILL.md)）的格式与中文风格。

## Current State Analysis

- 启动入口：[infrastructure/docker-compose.yml](file:///d:/projects/coder-micro-services/infrastructure/docker-compose.yml)（容器名前缀 `infrastructure-`，端口 3000/6001/6002/6003/6004/8500/3306/6379）。
- 自动建表：notification-service 在启动时通过 [services/notification-service/src/db.rs#L27-L89](file:///d:/projects/coder-micro-services/services/notification-service/src/db.rs#L27-L89) `ensure_schema()` 自动跑 `migrations/*.sql`；token / iam 走自身迁移逻辑（见各服务 `db.rs`）。表清空场景下首启即会重建。
- 环境检查脚本：[scripts/check-dev-env.js](file:///d:/projects/coder-micro-services/scripts/check-dev-env.js)，对应 `pnpm check:env`。
- 镜像同步：[scripts/base-images-sync.js](file:///d:/projects/coder-micro-services/scripts/base-images-sync.js)，对应 `pnpm images:sync`，参考 [coder-base-images-sync/SKILL.md](file:///d:/projects/coder-micro-services/.trae/skills/coder-base-images-sync/SKILL.md)。
- E2E：`tests/e2e` 自带 `resetBase` 会清表并导基础 seed（[shared/db/reset.ts](file:///d:/projects/coder-micro-services/tests/e2e/shared/db/reset.ts)），可作为最终冒烟。
- 已有相近 Skill：`coder-e2e-test`、`coder-base-images-sync`、`coder-service-audit`，均位于 `.trae/skills/<name>/SKILL.md`。本项目 `AGENTS.md` 中引用了占位 Skill 名 `coder-microservice-dev`，但**目录中并不存在**，因此采用更精确的 `coder-dev-bootstrap` 名称，避免覆盖未定义内容。
- 项目规则：PowerShell 输出、中文、根目录执行、不写死盘符（见 [.trae/rules/project_rules.md](file:///d:/projects/coder-micro-services/.trae/rules/project_rules.md)）。

## Proposed Changes

### 1. 新增文件 `.trae/skills/coder-dev-bootstrap/SKILL.md`

**why**：补齐「首次启动开发环境」标准流程，避免新人/AI 反复试错。

**what / how**：内容章节如下（细节均基于上节实际命令）。

#### Frontmatter

```yaml
name: "coder-dev-bootstrap"
description: "首次启动本地开发环境（拉取源码后 / 数据库已清空）。Invoke when 用户说 第一次启动、初始化开发环境、首启、bootstrap、数据库清空后重启。"
```

#### 章节大纲

1. **何时用**：第一次起 / 清库后重启 / CI 失败后想本地复现。
2. **前置**：
   - Docker Desktop 已启动
   - Node 18+ / pnpm
   - 根目录执行（`Get-Location` 校验 `infrastructure`、`scripts\check-dev-env.js`、`package.json` 存在）
3. **步骤**（PowerShell）：
   1. 同步并预拉基础镜像
      ```powershell
      pnpm images:sync
      ```
   2. 启动基础设施（首次会 build 业务镜像，需等待）
      ```powershell
      docker compose -f infrastructure/docker-compose.yml up -d --build
      ```
   3. 等待健康（轮询 mysql / redis / consul / token-service）
      ```powershell
      docker compose -f infrastructure/docker-compose.yml ps
      ```
      关键容器名：`infrastructure-mysql-1`、`infrastructure-redis-1`、`infrastructure-consul-1`、`infrastructure-token-service-1`、`infrastructure-iam-service-1`、`infrastructure-notification-service-1`、`infrastructure-api-gateway-1`。
   4. 自动建表说明：notification-service 启动时通过 `ensure_schema()` 应用 `services/notification-service/migrations/*.sql`；token-service / iam-service 走各自迁移逻辑。**清表后首启会自动重建**，无需手工执行 SQL。
   5. 环境检查
      ```powershell
      pnpm check:env
      ```
      期望：Docker Services 全部 healthy/running，HTTP 服务（gateway/token jwks/iam/notification/prue/consul）全部 OK。
   6. （可选）冒烟 E2E：自动注册 `e2eadmin/P@ssw0rd!` 并跑认证用例
      ```powershell
      pnpm -C tests/e2e install
      node scripts/run-e2e-local.js auth
      ```
   7. （可选）前端容器：`http://localhost:8080`（compose 已含 `frontend` 服务）；本地热更则在 `frontend/` 执行 `pnpm dev`。
4. **端口速查表**（与 e2e Skill 一致）：gateway 3000 / token 6001 / iam 6002 / notification 6003 / prue_backend 6004 / consul 8500 / mysql 3306 / redis 6379 / frontend 容器 8080 / frontend dev 8849。
5. **故障排查**：
   - 端口冲突：`Get-NetTCPConnection -LocalPort 3000`
   - 镜像拉不动：`pnpm images:sync` 重试，必要时按 [coder-base-images-sync](file:///d:/projects/coder-micro-services/.trae/skills/coder-base-images-sync/SKILL.md) 设 mirror
   - 容器一直 unhealthy：`docker logs <container>`，先关注 `infrastructure-mysql-1`
   - token-service 起不来 → api-gateway 卡在等 jwks：先看 token 日志
   - 重置环境：`docker compose -f infrastructure/docker-compose.yml down -v` 删卷后重启（再次清库）
6. **不做的事**：不改业务源码、不改 routes、不发布镜像、不跑全量 e2e。

## Assumptions & Decisions

- **Skill 命名**：使用 `coder-dev-bootstrap`，不复用 `AGENTS.md` 索引中的占位名 `coder-microservice-dev`（其目录与脚本 `scripts/dev-services.js` 在仓库中均不存在）。如用户要求改名，再调整。
- **不创建新脚本**：所有命令均复用现有 `package.json` script 与 `scripts/*.js`。
- **首启用 `--build`**：避免本地没有业务镜像导致 compose 拉私有 registry 失败。
- **不强依赖 e2e**：把 e2e 列为「可选验证步骤」，避免 Skill 变重。

## Verification

- 新建 SKILL.md 后：
  1. 文件结构符合 [skill-creator](file:///c:/Users/luqiz/.trae-cn/builtin/global/skills/skill-creator) 要求（frontmatter + 正文）。
  2. 按 SKILL 步骤在干净环境跑一遍，`pnpm check:env` 全绿。
  3. `node scripts/run-e2e-local.js auth` 通过。

## 待用户确认

- Skill 名称是否使用 `coder-dev-bootstrap`？（备选：`coder-microservice-dev` 以对齐 AGENTS.md 索引）
- 是否需要把「冒烟 E2E」从可选改为必选步骤？
