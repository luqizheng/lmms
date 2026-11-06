#!/usr/bin/env bash
# infrastructure/infra/start.sh —— 本目录 = 自包含部署单元（模式 C，docs/deploy-topologies.md）
# ─────────────────────────────────────────────────────────────────────────────
# 整目录 copy 到 infra 机后即可独立部署：
#   cp .env.example .env   # 按环境改值（中间件口令等）
#   ./start.sh             # 默认叠加 prod 片（收口端口、加 restart）
#   ./start.sh --remote    # 跨主机模式：以 remote 片发布宿主端口供他机访问
#   ./start.sh --dev       # 本地开发：以 dev 片发布宿主端口（供宿主进程连）
# 仓内运行时不拷贝也行：env 向上回退命中 infrastructure/.env（单一事实源）。
#
# 其余 args 透传到 docker compose up（如 --force-recreate）。
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT="superpowers-infra"
BASE="docker-compose-infra"
OVERLAY="prod"
UP_ARGS=()
for arg in "$@"; do
  case "$arg" in
    --remote)  OVERLAY="remote" ;;
    --dev)     OVERLAY="dev" ;;
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

# REQUIRED（infra 组裁剪；与根 scripts/start-prod.sh 同步）
for v in MYSQL_ROOT_PASSWORD MYSQL_USER MYSQL_PASSWORD RABBITMQ_DEFAULT_PASS; do
  [[ -z "${!v:-}" ]] && die "env var $v empty (set in .env)"
  [[ "${!v}" == "__SET_BY_OPS__" ]] && die "env var $v still __SET_BY_OPS__ placeholder"
done

docker network create superpowers >/dev/null 2>&1 || true

cd "$DIR"
log "up (overlay: $OVERLAY)..."
docker compose -p "$PROJECT" -f "$BASE.yml" -f "$BASE.$OVERLAY.yml" up -d ${UP_ARGS[@]+"${UP_ARGS[@]}"} \
  || { echo "[start:$PROJECT] FATAL: up failed" >&2; exit 20; }

n=$(docker compose -p "$PROJECT" ps --services 2>/dev/null | wc -l)
log "running: $n services — consul :8500 / mysql :3306 / redis :6379 / rabbitmq :5672"
