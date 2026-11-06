#!/usr/bin/env bash
# infrastructure/observability/start.sh —— 本目录 = 自包含部署单元（模式 C，docs/deploy-topologies.md）
# ─────────────────────────────────────────────────────────────────────────────
# 整目录 copy 到 obs 机后即可独立部署：
#   cp .env.example .env   # 按环境改值；跨主机需 CONSUL_HOST_IP=<infra 机 IP>
#   ./start.sh             # 默认叠加 prod 片：指标+链路 6 服务
#   ./start.sh --remote    # obs 与 infra 不同机时叠加 remote 片（extra_hosts 映射 consul）
#   ./start.sh --logs      # 叠加日志栈（loki + promtail，即 staging 片）；可与 --remote/--dev 叠加
#   ./start.sh --remote --logs   # 跨主机 + 日志全栈（8 服务）
# 仓内运行时不拷贝也行：env 向上回退命中 infrastructure/.env（单一事实源）。
#
# ⚠️ promtail 只发现**本机** docker 容器（logging=loki label）：obs 单独成机时，
#    它只收 obs 组自身容器日志；业务机日志进 Loki 需业务机侧 promtail sidecar
#    （LOKI_URL=http://<obs-ip>:3100，见 docs/deploy-topologies.md 已知边界）。
#
# 其余 args 透传到 docker compose up（如 --force-recreate）。
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT="superpowers-obs"
BASE="docker-compose-observability"
OVERLAY="prod"   # 基础选片：prod（默认）| remote | dev
LOGS=0           # --logs / --staging：叠加日志栈片（loki+promtail）
UP_ARGS=()
for arg in "$@"; do
  case "$arg" in
    --remote)         OVERLAY="remote" ;;
    --dev)            OVERLAY="dev" ;;
    --prod)           OVERLAY="prod" ;;
    --logs|--staging) LOGS=1 ;;
    --help|-h)        grep '^#' "$0" | sed '1d;$d' | sed 's/^# \{0,1\}//'; exit 0 ;;
    *)                UP_ARGS+=("$arg") ;;
  esac
done

log() { echo "[start:$PROJECT] $*"; }
die() { echo "[start:$PROJECT] FATAL: $*" >&2; exit 10; }

[[ -f "$DIR/$BASE.yml" ]] || die "$BASE.yml not found next to start.sh"
[[ -f "$DIR/$BASE.$OVERLAY.yml" ]] || die "overlay $BASE.$OVERLAY.yml not found"
if [[ "$LOGS" -eq 1 && ! -f "$DIR/$BASE.staging.yml" ]]; then
  die "--logs requires $BASE.staging.yml next to start.sh"
fi

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

# 跨主机（--remote）需 CONSUL_HOST_IP 指回 infra 机（compose 片内 :? 兜底）
if [[ "$OVERLAY" == "remote" && -z "${CONSUL_HOST_IP:-}" ]]; then
  die "CONSUL_HOST_IP empty — set it in .env (infra host IP for prometheus SD extra_hosts)"
fi

COMPOSE_FILES=(-f "$BASE.yml" -f "$BASE.$OVERLAY.yml")
if [[ "$LOGS" -eq 1 ]]; then
  COMPOSE_FILES+=(-f "$BASE.staging.yml")
fi

docker network create superpowers >/dev/null 2>&1 || true

cd "$DIR"
log "up (overlay: $OVERLAY${LOGS:+ + logs})..."
docker compose -p "$PROJECT" "${COMPOSE_FILES[@]}" up -d "${UP_ARGS[@]:-}" \
  || { echo "[start:$PROJECT] FATAL: up failed" >&2; exit 20; }

n=$(docker compose -p "$PROJECT" ps --services 2>/dev/null | wc -l)
log "running: $n services — grafana :3001 / jaeger :16686 / prometheus :9090${LOGS:+ / loki :3100}"
