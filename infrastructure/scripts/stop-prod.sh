#!/usr/bin/env bash
# infrastructure/scripts/stop-prod.sh
# ─────────────────────────────────────────────────────────────────────────────
# 停掉 prod 全栈（三组独立项目：services → observability → infra）。
# 保留 named volumes（数据不丢）。
#
# 用法：
#   ./infrastructure/scripts/stop-prod.sh             # 默认 down
#   ./infrastructure/scripts/stop-prod.sh --volumes   # 连 named volume 一起删
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INFRA_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$INFRA_DIR" || exit 1

# 与 start-prod.sh 的三段式项目名一一对应；顺序先 services（消费方）后 infra。
docker compose -p superpowers-svc -f docker-compose-services.yml -f docker-compose-services.prod.yml down "$@"
docker compose -p superpowers-obs -f docker-compose-observability.yml -f docker-compose-observability.prod.yml down "$@"
docker compose -p superpowers-infra -f docker-compose-infra.yml -f docker-compose-infra.prod.yml down "$@"
