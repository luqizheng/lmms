# Prod Compose Overlay Design — `docker-compose.prod.yml` + `start.sh` / `stop.sh`

**日期**：2026-07-06
**状态**：草案 → 待用户复审
**关联**：
- 实施范围：dev compose → 单 overlay 出 prod，对工作流零侵入
- 前序：`docs/superpowers/specs/2026-06-25-phase4-l1-production-readiness-design.md`（L1 baseline，health 优雅停机 / CVE / SLO / trace，未涉及 deploy overlay）

---

## 1. 背景与动机

12 周 roadmap + Phase 4 L1 production readiness 把"功能闭环 + 敢上 staging"做完了。当前 `infrastructure/docker-compose*.yml` 仍是 dev 形态：

| 现状 | 风险 |
|---|---|
| `docker-compose-infra.yml:50` `MYSQL_ROOT_PASSWORD: rootpass` 硬编码 | 仓库可见，prod 直接拿来用即破规 |
| infra 组件（mysql/redis/rabbitmq/consul）的 `ports:` 全暴露到 host | 任何能 reach host 的人都能直连 3306 等 |
| docker-compose-services.yml 把 `../docker-example/secrets` bind 进 `token-service` | dev 测试私钥进运行容器 |
| 8 个服务全无 `restart:` 策略 | 主机重启后服务全停 |
| token-service/Dockerfile:17 把 `private.pem` 烤进镜像 | ⚠️ 任何能 pull 镜像的人都能签 token（本次不修，独立 spec） |

但 `docker-compose.yml` 已经过历史 12 周 roadmap 沉淀、CI / e2e / `pnpm run dev` 整套都依赖它，**不能直接重构**。本次只做最小 prod overlay + 启动脚本，让 ops 用一行命令即可起 prod。

---

## 2. 范围

### In scope

1. 单文件 `infrastructure/docker-compose.prod.yml`：仅覆盖 dev → prod 三个差异轴（secrets / 端口暴露 / restart policy），不复制定义。
2. `infrastructure/start.sh` 与 `infrastructure/stop.sh`：mode 切换（`--dev` / 默认 prod）+ env 分层加载 + `-f` 多文件合成。
3. `infrastructure/.env.prod.example` + `.env.dev.example`：env 模板。
4. `.gitignore`：追加 `.env.prod` / `.env.dev`。
5. `infrastructure/docker-compose.dev.yml`：4 行 placeholder，留作未来 dev 增量；当前 `services: {}`。

### Out of scope（明确不做）

| 不做 | 原因 |
|---|---|
| Resource limits (mem/cpu) | 用户选最小化；超出 scope |
| read-only rootfs + cap-drop | 同上 |
| log driver (json-file → fluentbit) | 需新服务，超出 scope |
| nginx TLS terminator | L1 spec 推到 staging 后 |
| Helm / K8s manifest | L1 spec 明确 out-of-scope |
| Consul ACL token | 现状零 ACL 是 dev-by-design；阶段任务 |
| token-service Dockerfile 改（私钥烤进镜像） | 独立 security spec，本次仅在 README ⚠️ 标注 |

---

## 3. 核心设计：三个 dev → prod 差异轴

### 3.1 轴 A — Secrets → env vars

dev compose 把 secrets 硬编码 / bind-mount 进 container 配置；prod overlay 用 `${VAR:?required}` 从 `.env.prod` 注入。

| Service | dev 形态 | prod 覆盖 |
|---|---|---|
| mysql | `MYSQL_ROOT_PASSWORD: rootpass` | `MYSQL_ROOT_PASSWORD: ${MYSQL_ROOT_PASSWORD:?required}` |
| redis | 无密码 | 暂不加（dev/prod 行为一致，phase 5+ 跟 Consul ACL 一起处理） |
| rabbitmq | `guest:guest` 默认 | 暂不加（同上） |
| iam-service / token-service / audit-service | `DATABASE_PASSWORD: rootpass` | `DATABASE_PASSWORD: ${DB_PASSWORD:?required}` |
| 文件挂载 dev bind（`../docker-example/secrets:/app/keys`） | 透传给 token-service | `volumes: !override []` 清空（让镜像内的 dev 私钥不生效） |

**.env.prod.example 字段集**：
```bash
# infrastructure/.env.prod.example
MYSQL_ROOT_PASSWORD=__set_by_ops__
DB_PASSWORD=__set_by_ops__
# CONSUL_HTTP_TOKEN=       # 阶段 2，留空
# JWT_PRIVATE_KEY_B64=     # 阶段 2/3，留空
```

`.env.dev.example` 仅占位（4 行注释），所有 dev 行为仍由 dev compose 默认值提供。

### 3.2 轴 B — 端口暴露收敛

dev compose 把所有内部组件 bind 到 host。prod overlay 删除内部组件的 `ports:`：

| 组件 | dev 行为 | prod 行为 | 理由 |
|---|---|---|---|
| mysql :3306 | host 暴露 | **删** | DB 永远不该经 host |
| redis :6379 | host 暴露 | **删** | 同上 |
| rabbitmq :5672 / :15672 | host 暴露 | **删** | 同上 |
| consul :8500 / :8502 | host 暴露 | **删** | 通过 bastion `consul-cli` 调，运维 SOP 控制访问面 |
| sidecar :9081-9084 / :9087 | host 暴露 | **保留** | ops 需直连 `localhost:908X` 看 mesh 握手 |
| api-gateway :3000 | host 暴露 | **保留** | 公共入口 |
| frontend :8080 | host 暴露 | **保留** | 前端入口（HTTP；TLS 由后续 nginx terminator 处理） |
| observability (Prom 9090 / Grafana 3001 / Jaeger 16686) | host 暴露 | **保留** | observability 必须 ops 可达 |

**实现**：compose v2 `-f` 合并，**数组直接替换**。prod overlay 中对应服务块不写 `ports:` 字段即可——因为 base 写的是 list，prod 完全不写会**继承**。要清空必须显式写 `ports: !override []`：

```yaml
# infrastructure/docker-compose.prod.yml 节选
services:
  mysql:
    ports: !override []
  redis:
    ports: !override []
  rabbitmq:
    ports: !override []
  consul:
    ports: !override []
```

> **Compose 版本要求**：`!override` 标签需 compose v2.20+。镜像脚本里会 `docker compose version` 自查并 fail-fast。

### 3.3 轴 C — Restart policy

dev compose 几乎没有 restart policy；prod overlay 给所有长跑组件加 `restart: unless-stopped`：

- 8 个主服务 + 1 frontend + 2 outbox-publisher + 6 sidecar = 17 个容器全部 `restart: unless-stopped`
- consul / mysql / redis / rabbitmq 同上
- observability 容器（Prometheus/Grafana/Jaeger）由 ops 决定是否 restart，本次**不加**（避免 SLO 数据被覆盖/历史归零）

---

## 4. 启动脚本：`start.sh` / `stop.sh`

设计目标：单一入口、默认 prod、显式 `--dev`，env 分层、可被 CI / ops 复用。

### 4.1 start.sh

```bash
#!/bin/bash
# infrastructure/start.sh
# 启动 superpowers 部署。默认 prod；--dev 切 dev。
# 用法：./start.sh           # prod
#       ./start.sh --dev     # dev
#       ./start.sh --dev --force-recreate  # 透传其它 docker compose up 参数

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

mode="prod"
compose_args=()

for arg in "$@"; do
  case "$arg" in
    --dev)  mode="dev" ;;
    --prod) mode="prod" ;;
    -h|--help)
      sed -n '2,8p' "$0" | sed 's/^# \?//'
      exit 0 ;;
    *) compose_args+=("$arg") ;;
  esac
done

# env 分层：通用 .env 先入，mode-specific 覆盖
load_env() {
  local f="$1"
  [[ -f "$f" ]] || return 0
  set -a; source "$f"; set +a
}
load_env ".env"
load_env ".env.${mode}"

# compose 文件分层
files=(-f docker-compose.yml)
[[ -f "docker-compose.${mode}.yml" ]] && files+=(-f "docker-compose.${mode}.yml")

# compose 版本闸门（!override 需 v2.20+）
if ! docker compose version | grep -qE "v2\.(2[0-9]|[3-9][0-9])"; then
  echo "[start.sh] ERROR: docker compose v2.20+ required (current: $(docker compose version))" >&2
  exit 1
fi

docker compose "${files[@]}" up -d "${compose_args[@]}"

echo "[start.sh] mode=${mode} files=${files[*]}"
echo "[start.sh] running: $(docker compose "${files[@]}" ps --services 2>/dev/null | wc -l) services"
```

### 4.2 stop.sh

```bash
#!/bin/bash
# infrastructure/stop.sh
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

mode="prod"
[[ "${1:-}" == "--dev" ]] && mode="dev"

files=(-f docker-compose.yml)
[[ -f "docker-compose.${mode}.yml" ]] && files+=(-f "docker-compose.${mode}.yml")

docker compose "${files[@]}" down
```

### 4.3 与现存工作流兼容

| 调用方 | 命令 | 行为 |
|---|---|---|
| Ops 起 prod | `./infrastructure/start.sh` | compose v2.20 检查 → 加载 `.env` + `.env.prod` → `-f docker-compose.yml -f docker-compose.prod.yml up -d` |
| 工程师本地 | `./infrastructure/start.sh --dev` | 加载 `.env` + `.env.dev`（空）→ `-f docker-compose.yml -f docker-compose.dev.yml up -d`（dev overlay 为空等价于纯 base） |
| CI e2e | `docker compose -f infrastructure/docker-compose.yml -f infrastructure/docker-compose-observability.yml up -d` | **不变**，跟 start.sh 互不影响 |
| Ops 停 | `./infrastructure/stop.sh` | `down` （不删 named volume，与 dev 一致） |

---

## 5. 文件结构

```
infrastructure/
├── docker-compose.yml              ← 现状不变（dev base）
├── docker-compose-infra.yml         ← 现状不变
├── docker-compose-services.yml      ← 现状不变
├── docker-compose-observability.yml ← 现状不变
├── docker-compose.dev.yml          ← 新（4 行 placeholder）
├── docker-compose.prod.yml         ← 新（~80 行，本 spec 主体）
├── start.sh                         ← 新（25 行）
├── stop.sh                          ← 新（15 行）
├── .env.prod.example                ← 新（15 行）
├── .env.dev.example                 ← 新（4 行）
├── config/                          ← 现状不变
├── nginx/                           ← 现状不变
└── ...

.gitignore (root, 修改)
+ .env.prod
+ .env.dev
+ infrastructure/prod-secrets/
```

**新增 7 个文件、修改 1 个**。没有任何 file 删除或重命名。

---

## 6. 安全 caveat（本次不动，仅记录）

⚠️ **`token-service/Dockerfile:17` 把 `docker-example/secrets/private.pem` 烤进镜像**。

任何能 pull 镜像的人都能签 JWT。本次 prod overlay 仅在 `.env.prod.example` 中**留空一行占位**，并在 README / commit message 中复述 TODO。需要独立 security spec 处理：
- 让 token-service 接受 `TS_JWT_PRIVATE_KEY_PATH` 指向 compose secret 文件
- 或迁移到 KMS / Vault

不属本次 spec 范围。

---

## 7. Acceptance criteria

1. `./infrastructure/start.sh` 在 compose v2.20+ 环境无错启动 prod 全栈。
2. `./infrastructure/start.sh --dev` 行为与直接 `docker compose -f infrastructure/docker-compose.yml up -d` 一致。
3. 故意写错的 `.env.prod`（如 `MYSQL_ROOT_PASSWORD=wrongvalue`）启动 → compose fail-fast 在 mysql 启动前/启动时报错，而非接受默认。
4. `docker compose -f docker-compose.yml -f docker-compose.prod.yml config` 输出中：mysql/redis/rabbitmq/consul **不含** `ports:` 字段（`!override []` 已生效）。
5. 上述 `config` 输出：所有长跑 service 都有 `restart: unless-stopped`。
6. `node scripts/check-service-standard.js` 仍 PASS（不破坏服务规范）。
7. `pnpm run dev`、`pnpm run dev:infra` 现有命令**不受影响**。
8. CI e2e 现有调用栈 `docker compose -f infrastructure/docker-compose.yml ... up -d` **不受影响**。

---

## 8. ADR（推荐同时提交）

`docs-site/decisions/ADR-004-prod-compose-overlay.md`：记录"最小 prod overlay + 启动脚本"决策。原因：
- 跟 ADR-002-service-standard-baseline 同源（pod-level hardening）
- 把 out-of-scope 项（resource limits / TLS / K8s）显式记入"future decision"
- 提供给后续 phase 5/6 上 staging 时的回溯点

ADR 模板：

```markdown
# ADR-004: Production Compose Overlay + start.sh/stop.sh

## Context
12-week roadmap + L1 readiness 已交付，但 dev compose 仍是 dev 形态
（hardcoded secrets / 端口全暴露 / 无 restart）。需 ops 一行起 prod。

## Decision
单文件 docker-compose.prod.yml 覆盖三轴（secrets / ports / restart）。
start.sh / stop.sh 统一入口。dev base 不重构。

## Consequences
正向：dev 工作流零回归；overlay 文件 ~80 行可读
负向：dev/prod 双份文件需手工同步；未来 K8s 上需另起 spec
副作用：JWT 私钥 baked-in 仍未修（独立 spec）

## Out-of-scope 留待
- Resource limits / log driver / read-only rootfs
- nginx TLS terminator
- Helm/K8s manifest
- Consul ACL token
- token-service 私钥从镜像中剥离
```

可在 spec 批准后单开一个 commit / single PR 一起提交。

---

## 9. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| dev compose 改了字段没同步 prod overlay | ops 起 prod 时 drift 暴露 | Phase 5 加 CI smoke：每次改 compose 文件，两个 mode 都跑 up |
| `!override` 标签需 compose v2.20+ | 老 compose 默默忽略 → prod 启动时 hosts 仍有 mysql/3306 暴露 | start.sh fail-fast 检查版本 |
| `.env.prod` 误提交 | secrets 进仓库 | `.gitignore` 显式列；CI 加 `gitleaks` 兜底（独立工作） |
| 17 个 restart 策略手工同步易漏 | 漏一个就 prod 故障隐患 | 一次性本 spec 里写完 + ac §5 验证 |

---

## 10. Open questions

无。所有决策已通过 brainstorming 三轮澄清：
1. 形态：单文件 `docker-compose.prod.yml`
2. 范围：secrets / 端口 / restart
3. Secrets 形态：env vars + `.env.prod`
4. base 不重构：dev 覆盖留 placeholder

