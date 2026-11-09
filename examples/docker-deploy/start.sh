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
# env 加载（后者覆盖前者）：
#   1. .env      —— 环境配置（IP / 口令 / 拓扑），向上回退：
#                   $DIR/../../.env → $DIR/../.env → $DIR/.env
#   2. .env.prod —— 运维覆盖层（可选：服务器真实口令；缺失不阻塞）
#   3. .image.env —— **镜像钉版**（发布即固化，机器生成），最后加载 → 权威
# compose 的 image: 只引用 ${IMAGE_*}，值来自 .image.env；
# 缺变量时 compose 直接报错，本脚本也会在启动前拦下。
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

# ── 镜像钉版文件（.image.env）必须在 ─────────────────────────────────────────
# 缺失 = 不知道该拉哪个版本的镜像 → 启动前直接报错（compose 也会因 :? 报错）
IMAGE_ENV="$DIR/.image.env"
[[ -f "$IMAGE_ENV" ]] || die ".image.env not found in $DIR (生成：node scripts/update-image-env.js -v <ver>)"

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
# 1) 环境配置
load_env "$DIR/../../.env"
load_env "$DIR/../.env"
load_env "$DIR/.env"
# 2) 运维覆盖层（可选）
load_env "$DIR/../../.env.prod"
load_env "$DIR/../.env.prod"
load_env "$DIR/.env.prod"
# 3) 镜像钉版（最后加载 → 权威，.env 不能改掉已发布版本）
load_env "$DIR/../../.image.env"
load_env "$DIR/../.image.env"
load_env "$IMAGE_ENV"

# ── REQUIRED 环境变量校验 ────────────────────────────────────────────────────
# 镜像变量来自 .image.env（发布即固化）；其余来自 .env（环境配置）。
for v in DATABASE_URL RABBITMQ_DEFAULT_PASS \
         IAM_DEV_BT TOKEN_DEV_BT NOTIFICATION_DEV_BT AUDIT_DEV_BT \
         PRUE_DEV_BT GATEWAY_DEV_BT FILE_DEV_BT; do
  [[ -z "${!v:-}" ]] && die "env var $v empty (set in .env)"
  [[ "${!v}" == "__SET_BY_OPS__" ]] && die "env var $v still __SET_BY_OPS__ placeholder"
done

for v in IMAGE_API_GATEWAY IMAGE_TOKEN_SERVICE IMAGE_IAM_SERVICE \
         IMAGE_NOTIFICATION_SERVICE IMAGE_FILE_SERVICE IMAGE_AUDIT_SERVICE \
         IMAGE_PRUE_BACKEND; do
  [[ -z "${!v:-}" ]] && die "env var $v empty (set in .image.env)"
done

# ── compose env 文件（手工执行 compose 时也能复现同一份展开） ────────────────
# 进程环境已由上面的 load_env 注入（优先级高于 --env-file），这里只是兜底：
# 让直接跑 `docker compose ... config` 时也能拿到同样的变量。
COMPOSE_ENV_ARGS=()
for f in "$DIR/.env" "$DIR/.env.prod" "$IMAGE_ENV"; do
  [[ -f "$f" ]] && COMPOSE_ENV_ARGS+=(--env-file "$f")
done

# ── 启动 ─────────────────────────────────────────────────────────────────────
docker network create superpowers >/dev/null 2>&1 || true

cd "$DIR"
log "up (overlay: $OVERLAY, $BUILD_ARG)..."
docker compose -p "$PROJECT" "${COMPOSE_ENV_ARGS[@]+"${COMPOSE_ENV_ARGS[@]}"}" \
  -f "$BASE.yml" -f "$BASE.$OVERLAY.yml" \
  up -d $BUILD_ARG ${UP_ARGS[@]+"${UP_ARGS[@]}"} \
  || { echo "[start:$PROJECT] FATAL: up failed" >&2; exit 20; }

n=$(docker compose -p "$PROJECT" ps --services 2>/dev/null | wc -l)
log "running: $n services — api-gateway :3000 (health: curl http://<host>:3000/health)"
