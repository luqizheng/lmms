#!/usr/bin/env bash
# infrastructure/scripts/stop-prod.sh
# ─────────────────────────────────────────────────────────────────────────────
# 停 prod 栈（三组独立项目；默认全部，反序 services → observability → infra）。
# 保留 named volumes（数据不丢）。
#
# 用法：
#   ./infrastructure/scripts/stop-prod.sh             # 三组全停
#   ./infrastructure/scripts/stop-prod.sh svc infra   # 只停指定组（执行顺序仍按 svc→obs→infra 反序）
#   ./infrastructure/scripts/stop-prod.sh --volumes   # 连 named volume 一起删
#
# 注：down 的定位依据是 project 名 + 服务清单，prod/remote 片都只改
# ports/restart，不影响 down 范围——本脚本固定用 prod 片即可。
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INFRA_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$INFRA_DIR" || exit 1

log() { echo "[stop-prod] $*"; }

GROUPS=()
DOWN_ARGS=()
declare -A _seen=()
for arg in "$@"; do
  case "$arg" in
    infra)
      [[ -z "${_seen[infra]:-}" ]] && { GROUPS+=("infra"); _seen[infra]=1; } ;;
    obs|observability)
      [[ -z "${_seen[obs]:-}" ]] && { GROUPS+=("obs"); _seen[obs]=1; } ;;
    svc|services)
      [[ -z "${_seen[svc]:-}" ]] && { GROUPS+=("svc"); _seen[svc]=1; } ;;
    *) DOWN_ARGS+=("$arg") ;;
  esac
done
[[ ${#GROUPS[@]} -eq 0 ]] && GROUPS=(svc obs infra)

# 与 start-prod.sh 的三段式项目名一一对应；顺序先 services（消费方）后 infra。
stop_group() {
  case "$1" in
    svc)   log "down services..."
           docker compose -p superpowers-svc -f docker-compose-services.yml -f docker-compose-services.prod.yml down "${DOWN_ARGS[@]}" ;;
    obs)   log "down observability..."
           docker compose -p superpowers-obs -f docker-compose-observability.yml -f docker-compose-observability.prod.yml down "${DOWN_ARGS[@]}" ;;
    infra) log "down infra..."
           docker compose -p superpowers-infra -f docker-compose-infra.yml -f docker-compose-infra.prod.yml down "${DOWN_ARGS[@]}" ;;
  esac
}
for g in svc obs infra; do
  [[ " ${GROUPS[*]} " == *" $g "* ]] && stop_group "$g"
done
