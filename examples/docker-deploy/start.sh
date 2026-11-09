#!/usr/bin/env bash
# start.sh —— 本目录 = 自包含部署单元（模式 C）
# ─────────────────────────────────────────────────────────────────────────────
# 用法：
#   ./start.sh             # 默认 prod 片；自动检测：仓内 → --build，远程 → --no-build
#   ./start.sh --remote    # 跨主机片 + 自动 --no-build
#   ./start.sh --build     # 强制 build（覆盖自动检测）
#   ./start.sh --no-build  # 强制跳过 build（覆盖自动检测）
#   ./start.sh --force-recreate  # 透传其余 docker compose up 参数
#
# 启动前会向上回退加载 .env：$DIR/../../.env → $DIR/../.env → $DIR/.env
# （仓内命中 infrastructure/.env；拷贝到远程机后 .env 在本地覆盖）。
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT="superpowers-svc"
BASE="docker-compose-services"
OVERLAY="prod"
BUILD_ARG=""          # "", "--build", or "--no-build"
UP_ARGS=()

# ── 参数解析 ─────────────────────────────────────────────────────────────────
for arg in "$@"; do
  case "$arg" in
    --remote)   OVERLAY="remote" ;;
    --prod)     OVERLAY="prod" ;;
    --build)    BUILD_ARG="--build" ;;
    --no-build) BUILD_ARG="--no-build" ;;
    --help|-h)  grep '^#' "$0" | sed '1d;$d' | sed 's/^# \{0,1\}//'; exit 0 ;;
    *)          UP_ARGS+=("$arg") ;;
  esac
done

log()  { echo "[start:$PROJECT] $*"; }
die()  { echo "[start:$PROJECT] FATAL: $*" >&2; exit 10; }

# ── 基础文件校验 ─────────────────────────────────────────────────────────────
[[ -f "$DIR/$BASE.yml" ]] || die "$BASE.yml not found next to start.sh"
[[ -f "$DIR/$BASE.$OVERLAY.yml" ]] || die "overlay $BASE.$OVERLAY.yml not found"

# ── 自动 build / image 模式检测 ──────────────────────────────────────────────
# 判断依据：build.context 指向的服务 Dockerfile 是否存在。
# 仓内开发：$DIR/../../services/file-service/Dockerfile 存在 → --build
# 远程部署：该路径不存在（目录被拷贝出仓） → --no-build（用 registry 镜像）
if [[ -z "$BUILD_ARG" ]]; then
  if [[ -f "$DIR/../../services/file-service/Dockerfile" ]]; then
    BUILD_ARG="--build"
  else
    BUILD_ARG="--no-build"
  fi
fi
log "mode: $BUILD_ARG (overlay: $OVERLAY)"

# ── env 分层加载 ─────────────────────────────────────────────────────────────
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
# .env.prod 覆盖层（可选：服务器真实口令；缺失不阻塞）
load_env "$DIR/../../.env.prod"
load_env "$DIR/../.env.prod"
load_env "$DIR/.env.prod"

# ── REQUIRED 环境变量校验 ────────────────────────────────────────────────────
# 远程部署：image 由 kudi_repos + IMAGE_TAG_* 决定；REQUIRED 包含镜像 tag 变量。
# 仓内开发（--build）：build 不从 env 读 image，但 compose 里 image 字段仍需要这些变量。
for v in kudi_repos IMAGE_TAG_PG IMAGE_TAG_GATEWAY \
         DATABASE_URL RABBITMQ_DEFAULT_PASS \
         IAM_DEV_BT TOKEN_DEV_BT NOTIFICATION_DEV_BT AUDIT_DEV_BT \
         PRUE_DEV_BT GATEWAY_DEV_BT FILE_DEV_BT; do
  [[ -z "${!v:-}" ]] && die "env var $v empty (set in .env)"
  [[ "${!v}" == "__SET_BY_OPS__" ]] && die "env var $v still __SET_BY_OPS__ placeholder"
done

# ── 启动 ─────────────────────────────────────────────────────────────────────
docker network create superpowers >/dev/null 2>&1 || true

cd "$DIR"
log "up (overlay: $OVERLAY, $BUILD_ARG)..."
docker compose -p "$PROJECT" -f "$BASE.yml" -f "$BASE.$OVERLAY.yml" \
  up -d $BUILD_ARG ${UP_ARGS[@]+"${UP_ARGS[@]}"} \
  || { echo "[start:$PROJECT] FATAL: up failed" >&2; exit 20; }

n=$(docker compose -p "$PROJECT" ps --services 2>/dev/null | wc -l)
log "running: $n services — api-gateway :3000 (health: curl http://<host>:3000/health)"
