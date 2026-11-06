#!/usr/bin/env bash
# infrastructure/scripts/start-prod.sh
# ─────────────────────────────────────────────────────────────────────────────
# 三段式启动 superpowers prod（2026-11-05 拆分：infra / observability / services
# 各为独立 compose 项目，共享预建的 superpowers 网络）。配套 stop-prod.sh。
#
# 用法：
#   ./infrastructure/scripts/start-prod.sh                     # 标准 prod，三组全起
#   ./infrastructure/scripts/start-prod.sh infra svc           # 只起指定组（执行顺序仍按 infra→obs→svc）
#   ./infrastructure/scripts/start-prod.sh infra --remote      # 跨主机模式：点名的组以 *.remote.yml
#                                                              # 片代替 *.prod.yml 片（发布宿主端口）
#   ./infrastructure/scripts/start-prod.sh --pull              # 先 pull 再 up（强制刷新镜像）
#   ./infrastructure/scripts/start-prod.sh --force-recreate    # 其余 args 透传到 docker compose up
#
# 组名：infra | obs(observability) | svc(services)。
# 三组可分置不同服务器（模式 C，docs/deploy-topologies.md）：每台机只点自己的
# 组并加 --remote。跨主机 env 契约：
#   services 机：CONSUL_ADDR/REDIS_HOST/RABBITMQ_HOST/DATABASE_URL/
#                OTEL_EXPORTER_OTLP_ENDPOINT 指向他机 IP；SERVICE_ADDRESS=本机 LAN IP
#   obs 机：    CONSUL_HOST_IP=<infra 机 IP>（prometheus SD 与 consul-template
#               经 extra_hosts 映射 consul 容器名）
#   infra 机：  无跨主机需求
#
# 设计要点：
#   - 强制 .env.prod 存在且字段填好（fail-fast 在 compose 报错之前）；REQUIRED 按组裁剪
#   - compose v2.20+ 版本检查（!override 与 include 合并需要）
#   - env 分层加载：先 .env，再 .env.prod（后入覆盖前入）
#   - 启动顺序 infra → observability → services（obs 先于业务避免首启动丢 trace；
#     业务服务自带连接重试 + 自注册重试，无需跨组健康门）
#   - 共享网络 superpowers 为 external：不存在则预建（幂等）
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INFRA_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ENV_BASE="$INFRA_DIR/.env"
ENV_PROD="$INFRA_DIR/.env.prod"

cd "$INFRA_DIR" || { echo "[start-prod] FATAL: cannot cd to $INFRA_DIR" >&2; exit 10; }

die() { echo "[start-prod] FATAL: $*" >&2; exit "${2:-10}"; }
log() { echo "[start-prod] $*"; }

# 0. --help 优先处理（不依赖 .env.prod 存在与 docker 守护）
for arg in "$@"; do
  case "$arg" in
    --help|-h)
      grep '^#' "$0" | sed '1d;$d' | sed 's/^# \{0,1\}//'
      exit 0 ;;
  esac
done

# 1. .env.prod 必须存在
[[ -f "$ENV_PROD" ]] || die "$ENV_PROD not found — copy from .env.prod.example and fill values" 10

# 2. compose v2.20+ 检查
if ! docker compose version | grep -qE 'v?2\.(2[0-9]|[3-9][0-9])'; then
  die "docker compose v2.20+ required (found: $(docker compose version))" 10
fi
log "docker compose version: $(docker compose version | head -1)"

# 3. env 分层加载（base → prod 覆盖）
load_env() {
  local f="$1"
  [[ -f "$f" ]] || return 0
  set +u
  # shellcheck disable=SC1090
  source "$f"
  set -u
}
load_env "$ENV_BASE"
load_env "$ENV_PROD"

# 4. 参数解析：组选择（去重保序）+ --remote + 透传
GROUPS=()
COMPOSE_ARGS=()
DO_PULL=0
REMOTE=0
declare -A _seen_group=()
for arg in "$@"; do
  case "$arg" in
    infra)
      [[ -z "${_seen_group[infra]:-}" ]] && { GROUPS+=("infra"); _seen_group[infra]=1; } ;;
    obs|observability)
      [[ -z "${_seen_group[obs]:-}" ]] && { GROUPS+=("obs"); _seen_group[obs]=1; } ;;
    svc|services)
      [[ -z "${_seen_group[svc]:-}" ]] && { GROUPS+=("svc"); _seen_group[svc]=1; } ;;
    --remote) REMOTE=1 ;;
    --pull)   DO_PULL=1 ;;
    --help|-h)
      grep '^#' "$0" | sed '1d;$d' | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *)        COMPOSE_ARGS+=("$arg") ;;
  esac
done
[[ ${#GROUPS[@]} -eq 0 ]] && GROUPS=(infra obs svc)

# 5. 校验关键 vars（REQUIRED 按组裁剪；全组清单与 deploy-remote.sh 同步）
require_vars() {
  local REQUIRED
  case "$1" in
    infra) REQUIRED=(MYSQL_ROOT_PASSWORD MYSQL_USER MYSQL_PASSWORD RABBITMQ_DEFAULT_PASS) ;;
    svc)   REQUIRED=(DATABASE_URL RABBITMQ_DEFAULT_PASS \
                    IAM_DEV_BT TOKEN_DEV_BT NOTIFICATION_DEV_BT AUDIT_DEV_BT \
                    PRUE_DEV_BT GATEWAY_DEV_BT FILE_DEV_BT) ;;
    obs)   return 0 ;;
  esac
  local v
  for v in "${REQUIRED[@]}"; do
    [[ -z "${!v:-}" ]] && die "env var $v empty after .env.prod load (required by group: $1)" 10
    [[ "${!v}" == "__SET_BY_OPS__" ]] && die "env var $v still __SET_BY_OPS__ placeholder (group: $1)" 10
  done
}
for g in "${GROUPS[@]}"; do require_vars "$g"; done

# 6. 共享网络预建（幂等；三组 compose 均以 external 引用）
docker network create superpowers >/dev/null 2>&1 || true

# 7. compose 命令构造（--remote 时以 remote 片代替 prod 片）
OVL="prod"
[[ "$REMOTE" -eq 1 ]] && OVL="remote"
DC_INFRA=(docker compose -p superpowers-infra -f infra/docker-compose-infra.yml -f "infra/docker-compose-infra.$OVL.yml")
DC_OBS=(docker compose -p superpowers-obs -f observability/docker-compose-observability.yml -f "observability/docker-compose-observability.$OVL.yml")
DC_SVC=(docker compose -p superpowers-svc -f services/docker-compose-services.yml -f "services/docker-compose-services.$OVL.yml")
log "overlay: .$OVL.yml slices; groups: ${GROUPS[*]}"

pull_group() {
  case "$1" in
    infra) "${DC_INFRA[@]}" pull || die "infra pull failed" 20 ;;
    obs)   "${DC_OBS[@]}" pull || die "observability pull failed" 20 ;;
    svc)   "${DC_SVC[@]}" pull || die "services pull failed" 20 ;;
  esac
}
up_group() {
  case "$1" in
    infra) log "[*] infra (consul / mysql / redis / rabbitmq)..."
           "${DC_INFRA[@]}" up -d "${COMPOSE_ARGS[@]}" || die "infra up failed" 20 ;;
    obs)   log "[*] observability (otel / jaeger / prometheus / grafana)..."
           "${DC_OBS[@]}" up -d "${COMPOSE_ARGS[@]}" || die "observability up failed" 20 ;;
    svc)   log "[*] services (业务服务 + outbox publishers)..."
           "${DC_SVC[@]}" up -d "${COMPOSE_ARGS[@]}" || die "services up failed" 20 ;;
  esac
}
count_group() {
  case "$1" in
    infra) "${DC_INFRA[@]}" ps --services 2>/dev/null | wc -l ;;
    obs)   "${DC_OBS[@]}" ps --services 2>/dev/null | wc -l ;;
    svc)   "${DC_SVC[@]}" ps --services 2>/dev/null | wc -l ;;
  esac
}
group_selected() { [[ " ${GROUPS[*]} " == *" $1 "* ]]; }

# 8. pull（可选）与固定顺序 up（infra → obs → svc）
[[ "$DO_PULL" -eq 1 ]] && for g in infra obs svc; do
  group_selected "$g" && pull_group "$g"
done
for g in infra obs svc; do
  group_selected "$g" && up_group "$g"
done

# 9. 汇总与端点提示（只覆盖本次选中的组）
TOTAL=0
PARTS=""
for g in infra obs svc; do
  if group_selected "$g"; then
    n=$(count_group "$g")
    TOTAL=$((TOTAL + n))
    PARTS="$PARTS $g=$n"
  fi
done
log "running: $TOTAL services across selected groups ($PARTS)"
if group_selected svc; then
  log "public endpoint: api-gateway:3000 — health: curl http://<host>:3000/health"
fi
if group_selected obs; then
  log "observability: grafana :3001 / jaeger :16686 / prometheus :9090"
fi
