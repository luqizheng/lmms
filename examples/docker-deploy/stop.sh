#!/usr/bin/env bash
# stop.sh —— 停止本目录部署单元
# ─────────────────────────────────────────────────────────────────────────────
# 用法：
#   ./stop.sh              # down（保留 named volumes）
#   ./stop.sh --volumes    # 连 named volume 一起删
#   ./stop.sh --help       # 显示帮助
#
# 注意：down 的定位依据是 project 名 + service 名，与启动时用的 overlay 片无关。
# start.sh 用的是 prod 还是 remote，stop.sh 都能正确停止所有容器。
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT="superpowers-svc"
BASE="docker-compose-services"

cd "$DIR"
echo "[stop:$PROJECT] down..."
docker compose -p "$PROJECT" -f "$BASE.yml" down "$@"
