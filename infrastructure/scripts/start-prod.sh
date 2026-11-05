#!/usr/bin/env bash
# infrastructure/scripts/start-prod.sh
# ─────────────────────────────────────────────────────────────────────────────
# 三段式启动 superpowers prod（2026-11-05 拆分：infra / observability / services
# 各为独立 compose 项目，共享预建的 superpowers 网络）。配套 stop-prod.sh。
#
# 用法：
#   ./infrastructure/scripts/start-prod.sh                    # 标准 prod
#   ./infrastructure/scripts/start-prod.sh --force-recreate   # 传透到 docker compose up
#   ./infrastructure/scripts/start-prod.sh --pull             # 先 pull 再 up（强制刷新镜像）
#
# 设计要点：
#   - 强制 .env.prod 存在且字段填好（fail-fast 在 compose 报错之前）
#   - compose v2.20+ 版本检查（!override 与 include 合并需要）
#   - env 分层加载：先 .env，再 .env.prod（后入覆盖前入）
#   - 三段启动顺序：infra（consul/mysql/redis/rabbitmq 健康就绪）→
#     observability（otel/jaeger/prometheus 先于业务起，避免首启动丢 trace）→
#     services（业务服务各自带连接重试 + 自注册重试，无需健康门）
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

# 4. 校验关键 vars（与 deploy-remote.sh 同步）
for v in MYSQL_ROOT_PASSWORD MYSQL_USER MYSQL_PASSWORD DATABASE_URL RABBITMQ_DEFAULT_PASS \
         IAM_DEV_BT TOKEN_DEV_BT NOTIFICATION_DEV_BT AUDIT_DEV_BT \
         PRUE_DEV_BT GATEWAY_DEV_BT FILE_DEV_BT; do
  if [[ -z "${!v:-}" ]]; then
    die "env var $v empty after .env.prod load" 10
  fi
  if [[ "${!v}" == "__SET_BY_OPS__" ]]; then
    die "env var $v still __SET_BY_OPS__ placeholder" 10
  fi
done

# 5. 透传剩余 args
COMPOSE_ARGS=()
DO_PULL=0
for arg in "$@"; do
  case "$arg" in
    --pull)         DO_PULL=1 ;;
    --help|-h)
      sed -n '2,16p' "$0" | sed 's/^# \?//'
      exit 0 ;;
    *)              COMPOSE_ARGS+=("$arg") ;;
  esac
done

# 6. 共享网络预建（幂等；三组 compose 均以 external 引用）
docker network create superpowers >/dev/null 2>&1 || true

# 7. 三段式 up
DC_INFRA=(docker compose -p superpowers-infra -f docker-compose-infra.yml -f docker-compose-infra.prod.yml)
DC_OBS=(docker compose -p superpowers-obs -f docker-compose-observability.yml -f docker-compose-observability.prod.yml)
DC_SVC=(docker compose -p superpowers-svc -f docker-compose-services.yml -f docker-compose-services.prod.yml)

[[ "$DO_PULL" -eq 1 ]] && {
  "${DC_INFRA[@]}" pull || die "infra pull failed" 20
  "${DC_OBS[@]}" pull || die "observability pull failed" 20
  "${DC_SVC[@]}" pull || die "services pull failed" 20
}

log "[1/3] infra (consul / mysql / redis / rabbitmq)..."
"${DC_INFRA[@]}" up -d "${COMPOSE_ARGS[@]}" || die "infra up failed" 20

log "[2/3] observability (otel / jaeger / prometheus / grafana)..."
"${DC_OBS[@]}" up -d "${COMPOSE_ARGS[@]}" || die "observability up failed" 20

log "[3/3] services (8 业务服务 + outbox publishers)..."
"${DC_SVC[@]}" up -d "${COMPOSE_ARGS[@]}" || die "services up failed" 20

N1=$("${DC_INFRA[@]}" ps --services 2>/dev/null | wc -l)
N2=$("${DC_OBS[@]}" ps --services 2>/dev/null | wc -l)
N3=$("${DC_SVC[@]}" ps --services 2>/dev/null | wc -l)
log "running: $((N1 + N2 + N3)) services across 3 groups (infra=$N1 obs=$N2 svc=$N3)"
log "public endpoint: api-gateway:3000（合体模式经 frontend nginx :8080；三组模式按网关发布口）"
log "health: curl http://<host>:3000/health"
