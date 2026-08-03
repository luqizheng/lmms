#!/usr/bin/env bash
# infrastructure/scripts/stop-prod.sh
# ─────────────────────────────────────────────────────────────────────────────
# 停掉 prod 全栈。保留 named volumes（数据不丢）。
#
# 用法：
#   ./infrastructure/scripts/stop-prod.sh             # 默认 down
#   ./infrastructure/scripts/stop-prod.sh --volumes   # 连 named volume 一起删
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INFRA_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
COMPOSE_BASE="$INFRA_DIR/docker-compose.yml"
COMPOSE_PROD="$INFRA_DIR/docker-compose.prod.yml"

cd "$INFRA_DIR" || exit 1

docker compose -f "$COMPOSE_BASE" -f "$COMPOSE_PROD" down "$@"
