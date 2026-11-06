#!/usr/bin/env bash
# infrastructure/services/start.sh —— 本目录 = 自包含部署单元（模式 C，docs/deploy-topologies.md）
# ─────────────────────────────────────────────────────────────────────────────
# 整目录 copy 到 services 机后即可独立部署（镜像引用形态——仓内 build 形态的
# context 指向仓根，拷出后不可构建，部署本就走 registry 镜像）：
#   cp .env.example .env   # 按环境改值：DATABASE_URL/CONSUL_ADDR/SERVICE_ADDRESS 等
#                          #   全指他机 IP（跨主机契约见 docs/deploy-topologies.md）
#   ./start.sh             # 默认叠加 prod 片（收口端口、加 restart）
#   ./start.sh --remote    # 跨主机模式：以 remote 片补 restart（base 端口已发布）
# 仓内运行时不拷贝也行：env 向上回退命中 infrastructure/.env（单一事实源）。
#
# 其余 args 透传到 docker compose up（如 --force-recreate）。
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT="superpowers-svc"
BASE="docker-compose-services"
OVERLAY="prod"
UP_ARGS=()
for arg in "$@"; do
  case "$arg" in
    --remote)  OVERLAY="remote" ;;
    --prod)    OVERLAY="prod" ;;
    --help|-h) grep '^#' "$0" | sed '1d;$d' | sed 's/^# \{0,1\}//'; exit 0 ;;
    *)         UP_ARGS+=("$arg") ;;
  esac
done

log() { echo "[start:$PROJECT] $*"; }
die() { echo "[start:$PROJECT] FATAL: $*" >&2; exit 10; }

[[ -f "$DIR/$BASE.yml" ]] || die "$BASE.yml not found next to start.sh"
[[ -f "$DIR/$BASE.$OVERLAY.yml" ]] || die "overlay $BASE.$OVERLAY.yml not found"

# env 分层加载：祖先目录先加载（仓内=根 .env），本地后加载（拷出后覆盖）
load_env() {
  local f="$1"
  [[ -f "$f" ]] || return 0
  set +u; set -a
  # shellcheck disable=SC1090
  source "$f"
  set +a; set -u
  log "env: $f"
}
load_env "$DIR/../../.env"
load_env "$DIR/../.env"
load_env "$DIR/.env"

# REQUIRED（services 组裁剪；与根 scripts/start-prod.sh 同步）
for v in DATABASE_URL RABBITMQ_DEFAULT_PASS \
         IAM_DEV_BT TOKEN_DEV_BT NOTIFICATION_DEV_BT AUDIT_DEV_BT \
         PRUE_DEV_BT GATEWAY_DEV_BT FILE_DEV_BT; do
  [[ -z "${!v:-}" ]] && die "env var $v empty (set in .env)"
  [[ "${!v}" == "__SET_BY_OPS__" ]] && die "env var $v still __SET_BY_OPS__ placeholder"
done

docker network create superpowers >/dev/null 2>&1 || true

cd "$DIR"
log "up (overlay: $OVERLAY)..."
docker compose -p "$PROJECT" -f "$BASE.yml" -f "$BASE.$OVERLAY.yml" up -d "${UP_ARGS[@]:-}" \
  || { echo "[start:$PROJECT] FATAL: up failed" >&2; exit 20; }

n=$(docker compose -p "$PROJECT" ps --services 2>/dev/null | wc -l)
log "running: $n services — api-gateway :3000（health: curl http://<host>:3000/health）"
