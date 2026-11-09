#!/usr/bin/env bash
# infrastructure/lmms-mysql-dev/start.sh —— lmms 业务服务组（MySQL）本机开发启动入口
# ─────────────────────────────────────────────────────────────────────────────
# 前置：先启动 ../infra-mysql（consul / mysql / redis / rabbitmq 都在那里）：
#     (cd ../infra-mysql && ./start.sh --dev)
#
# 用法：
#   ./start.sh              # 叠加 dev 片（发布各服务调试端口）
#   ./start.sh --no-overlay # 只起基座（仅 api-gateway:3000 对外）
#   ./start.sh --build      # 其余参数透传 docker compose up
#
# 环境变量：仅本目录 .env（cp .env.example .env），不向上回退。
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT="lmms-mysql-dev"
BASE="docker-compose-lmms"
OVERLAY="dev"
UP_ARGS=()
for arg in "$@"; do
  case "$arg" in
    --no-overlay) OVERLAY="" ;;
    --help|-h) grep '^#' "$0" | sed '1d;$d' | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) UP_ARGS+=("$arg") ;;
  esac
done

log() { echo "[start:$PROJECT] $*"; }
die() { echo "[start:$PROJECT] FATAL: $*" >&2; exit 10; }

[[ -f "$DIR/$BASE.yml" ]] || die "$BASE.yml not found next to start.sh"
[[ -z "$OVERLAY" || -f "$DIR/$BASE.$OVERLAY.yml" ]] || die "overlay $BASE.$OVERLAY.yml not found"
[[ -f "$DIR/.env" ]] || die ".env not found (copy .env.example to .env)"

set +u; set -a
# shellcheck disable=SC1090
source "$DIR/.env"
set +a; set -u
log "env: $DIR/.env"

for v in DATABASE_URL RABBITMQ_DEFAULT_PASS \
         IAM_DEV_BT TOKEN_DEV_BT NOTIFICATION_DEV_BT AUDIT_DEV_BT \
         PRUE_DEV_BT GATEWAY_DEV_BT FILE_DEV_BT; do
  [[ -z "${!v:-}" ]] && die "env var $v empty (set in .env)"
  [[ "${!v}" == "__SET_BY_OPS__" ]] && die "env var $v still __SET_BY_OPS__ placeholder"
done

# ── 前置：infra 必须已启动 ──
docker network ls --format '{{.Name}}' | grep -qx 'superpowers' \
  || die "docker network 'superpowers' 不存在——请先启动 ../infra-mysql"
docker ps --filter "label=com.docker.compose.service=consul" --filter "status=running" -q | grep -q . \
  || die "consul 未运行——请先启动 ../infra-mysql（./start.sh --dev）"
docker ps --filter "label=com.docker.compose.service=mysql" --filter "status=running" -q | grep -q . \
  || die "mysql 未运行——请先启动 ../infra-mysql（./start.sh --dev）"

cd "$DIR"
FILES=(-f "$BASE.yml")
[[ -n "$OVERLAY" ]] && FILES+=(-f "$BASE.$OVERLAY.yml")
log "up (overlay: ${OVERLAY:-none})..."
docker compose -p "$PROJECT" "${FILES[@]}" up -d ${UP_ARGS[@]+"${UP_ARGS[@]}"} \
  || { echo "[start:$PROJECT] FATAL: up failed" >&2; exit 20; }

log "running — api-gateway :3000 (health: curl http://127.0.0.1:3000/health)"
[[ -n "$OVERLAY" ]] && log "debug ports — token:8082 iam:9081 ntf:8083 audit:8084 file:8086 prue:8087"
