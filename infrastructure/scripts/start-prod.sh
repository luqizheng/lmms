#!/usr/bin/env bash
# infrastructure/scripts/start-prod.sh
# ─────────────────────────────────────────────────────────────────────────────
# 单入口启动 superpowers prod。配套 stop-prod.sh。
#
# 用法：
#   ./infrastructure/scripts/start-prod.sh                    # 标准 prod
#   ./infrastructure/scripts/start-prod.sh --force-recreate   # 传透到 docker compose up
#   ./infrastructure/scripts/start-prod.sh --pull             # 先 pull 再 up（强制刷新镜像）
#
# 设计要点：
#   - 强制 .env.prod 存在且字段填好（fail-fast 在 compose 报错之前）
#   - compose v2.20+ 版本检查（!override 需要）
#   - env 分层加载：先 .env，再 .env.prod（后入覆盖前入）
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INFRA_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ENV_BASE="$INFRA_DIR/.env"
ENV_PROD="$INFRA_DIR/.env.prod"
COMPOSE_BASE="$INFRA_DIR/docker-compose.yml"
COMPOSE_PROD="$INFRA_DIR/docker-compose.prod.yml"

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
         PRUE_DEV_BT GATEWAY_DEV_BT CONFIG_DEV_BT FILE_DEV_BT; do
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
      sed -n '2,12p' "$0" | sed 's/^# \?//'
      exit 0 ;;
    *)              COMPOSE_ARGS+=("$arg") ;;
  esac
done

# 6. up
[[ "$DO_PULL" -eq 1 ]] && docker compose -f "$COMPOSE_BASE" -f "$COMPOSE_PROD" pull
docker compose -f "$COMPOSE_BASE" -f "$COMPOSE_PROD" up -d "${COMPOSE_ARGS[@]}" \
  || die "docker compose up failed" 20

log "running: $(docker compose -f "$COMPOSE_BASE" -f "$COMPOSE_PROD" ps --services 2>/dev/null | wc -l) services"
log "public endpoint: http://<host>:8080 (frontend nginx → api-gateway:3000)"
log "private: api-gateway:3000 is NOT published to host (container network only)"
