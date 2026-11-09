#!/usr/bin/env bash
# start.sh —— 全栈自包含部署单元（infra + observability + services）
# ─────────────────────────────────────────────────────────────────────────────
# 用法：
#   ./start.sh             # prod 片（业务端口只留 gateway :3000）；仓内自动 --build
#   ./start.sh --remote    # 跨主机分置片（保留业务端口供 consul 回拨）+ 自动 --no-build
#   ./start.sh --logs      # 叠加日志栈（loki + promtail）
#   ./start.sh --build     # 强制 build（覆盖自动检测）
#   ./start.sh --no-build  # 强制跳过 build（覆盖自动检测）
#   ./start.sh --force-recreate  # 透传其余 docker compose up 参数
#
# env 加载（后者覆盖前者）：
#   1. .env       —— 环境配置（IP / 口令 / 拓扑）
#                  向上回退：$DIR/../../.env → $DIR/../.env → $DIR/.env
#   2. .env.prod  —— 运维覆盖层（可选；缺失不阻塞）
#   3. .image.env —— **镜像钉版**（平台镜像机器生成 + 基础镜像手工维护），最后加载 → 权威
#
# compose 的 image: 只引用 ${IMAGE_*}，值来自 .image.env；缺变量直接报错。
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT="superpowers-full"
BASE="docker-compose-services"
OVERLAY="prod"
BUILD_ARG=""
WITH_LOGS=0
UP_ARGS=()

for arg in "$@"; do
  case "$arg" in
    --remote)   OVERLAY="remote" ;;
    --prod)     OVERLAY="prod" ;;
    --logs)     WITH_LOGS=1 ;;
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

IMAGE_ENV="$DIR/.image.env"
[[ -f "$IMAGE_ENV" ]] || die ".image.env not found in $DIR（生成：node scripts/update-image-env.js）"

# ── 自动 build / image 模式检测 ──────────────────────────────────────────────
# 仓内开发：$DIR/../../services/file-service/Dockerfile 存在 → --build
# 远程部署：该路径不存在（目录被拷出仓） → --no-build（用 registry 镜像）
if [[ -z "$BUILD_ARG" ]]; then
  if [[ -f "$DIR/../../services/file-service/Dockerfile" ]]; then
    BUILD_ARG="--build"
  else
    BUILD_ARG="--no-build"
  fi
fi
log "mode: $BUILD_ARG (overlay: $OVERLAY, logs: $WITH_LOGS)"

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
load_env "$DIR/../../.env.prod"
load_env "$DIR/../.env.prod"
load_env "$DIR/.env.prod"
load_env "$DIR/../../.image.env"
load_env "$DIR/../.image.env"
load_env "$IMAGE_ENV"

# ── REQUIRED 校验 ────────────────────────────────────────────────────────────
for v in DATABASE_URL RABBITMQ_DEFAULT_PASS POSTGRES_PASSWORD \
         IAM_DEV_BT TOKEN_DEV_BT NOTIFICATION_DEV_BT AUDIT_DEV_BT \
         PRUE_DEV_BT GATEWAY_DEV_BT FILE_DEV_BT; do
  [[ -z "${!v:-}" ]] && die "env var $v empty (set in .env)"
  [[ "${!v}" == "__SET_BY_OPS__" ]] && die "env var $v still __SET_BY_OPS__ placeholder"
done

for v in IMAGE_API_GATEWAY IMAGE_TOKEN_SERVICE IMAGE_IAM_SERVICE \
         IMAGE_NOTIFICATION_SERVICE IMAGE_FILE_SERVICE IMAGE_AUDIT_SERVICE \
         IMAGE_PRUE_BACKEND IMAGE_CONSUL IMAGE_REDIS IMAGE_RABBITMQ IMAGE_POSTGRES \
         IMAGE_PROMETHEUS IMAGE_ALERTMANAGER IMAGE_GRAFANA IMAGE_CONSUL_TEMPLATE \
         IMAGE_OTEL_COLLECTOR IMAGE_JAEGER; do
  [[ -z "${!v:-}" ]] && die "env var $v empty (set in .image.env)"
done
if [[ "$WITH_LOGS" == "1" ]]; then
  for v in IMAGE_LOKI IMAGE_PROMTAIL; do
    [[ -z "${!v:-}" ]] && die "env var $v empty (set in .image.env; needed by --logs)"
  done
fi

# ── compose env 文件（手工执行 compose 时也能复现同一份展开） ────────────────
COMPOSE_ENV_ARGS=()
for f in "$DIR/.env" "$DIR/.env.prod" "$IMAGE_ENV"; do
  [[ -f "$f" ]] && COMPOSE_ENV_ARGS+=(--env-file "$f")
done

# ── compose 文件链 ───────────────────────────────────────────────────────────
COMPOSE_FILES=(-f docker-compose.yml -f "$BASE.$OVERLAY.yml")
[[ "$WITH_LOGS" == "1" ]] && COMPOSE_FILES+=(-f docker-compose-observability.logs.yml)

# ── 启动 ─────────────────────────────────────────────────────────────────────
docker network create superpowers >/dev/null 2>&1 || true

cd "$DIR"
log "up (overlay: $OVERLAY, $BUILD_ARG)..."
docker compose -p "$PROJECT" "${COMPOSE_ENV_ARGS[@]+"${COMPOSE_ENV_ARGS[@]}"}" \
  "${COMPOSE_FILES[@]}" \
  up -d $BUILD_ARG ${UP_ARGS[@]+"${UP_ARGS[@]}"} \
  || { echo "[start:$PROJECT] FATAL: up failed" >&2; exit 20; }

n=$(docker compose -p "$PROJECT" ps --services 2>/dev/null | wc -l)
log "running: $n services — gateway :3000 | grafana :3001 | prometheus :9090 | jaeger :16686 | consul :8500"
