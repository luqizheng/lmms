#!/usr/bin/env bash
# infrastructure/infra/start.sh —— 本目录 = 自包含部署单元（模式 C，docs/deploy-topologies.md）
# ─────────────────────────────────────────────────────────────────────────────
# 整目录 copy 到 infra 机后即可独立部署：
#   cp .env.example .env   # 按环境改值（中间件口令等）
#   ./start.sh             # 默认叠加 prod 片（收口端口、加 restart）
#   ./start.sh --remote    # 跨主机模式：以 remote 片发布宿主端口供他机访问
#   ./start.sh --dev       # 本地开发：以 dev 片发布宿主端口（供宿主进程连）
#
# 环境变量来源：仅本目录下的 .env（必需）与可选的 .env.prod（生产口令覆盖），
# 不再向上回退 infrastructure/.env 或仓库根 .env，确保本部署单元可完全独立。
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

# env 仅从本目录加载：.env 必需，.env.prod 可选（生产口令覆盖）
# 不再向上回退，确保 infra 部署单元可完全独立拷贝运行。
load_env() {
  local f="$1"
  [[ -f "$f" ]] || return 0
  set +u; set -a
  # shellcheck disable=SC1090
  source "$f"
  set +a; set -u
  log "env: $f"
}
[[ -f "$DIR/.env" ]] || die ".env not found in infra dir (cp .env.example .env)"
load_env "$DIR/.env"
# prod 覆盖层（可选：服务器真实口令在 .env.prod；缺失时保持 .env 值，不阻塞）
load_env "$DIR/.env.prod"

# REQUIRED（infra 组裁剪；与根 scripts/start-prod.sh 同步）
for v in POSTGRES_PASSWORD RABBITMQ_DEFAULT_PASS; do
  [[ -z "${!v:-}" ]] && die "env var $v empty (set in .env)"
  [[ "${!v}" == "__SET_BY_OPS__" ]] && die "env var $v still __SET_BY_OPS__ placeholder"
done

docker network create superpowers >/dev/null 2>&1 || true

cd "$DIR"
log "up (overlay: $OVERLAY)..."
docker compose -p "$PROJECT" -f "$BASE.yml" -f "$BASE.$OVERLAY.yml" up -d ${UP_ARGS[@]+"${UP_ARGS[@]}"} \
  || { echo "[start:$PROJECT] FATAL: up failed" >&2; exit 20; }

n=$(docker compose -p "$PROJECT" ps --services 2>/dev/null | wc -l)
log "running: $n services — consul :8500 / postgres :5432 / redis :6379 / rabbitmq :5672"
