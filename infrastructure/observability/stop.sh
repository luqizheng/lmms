#!/usr/bin/env bash
# infrastructure/observability/stop.sh —— 本目录部署单元的停止入口
# 用法：./stop.sh              # down（保留 volumes）
#       ./stop.sh --volumes    # 连 named volume 一起删
# down 的定位依据是 project 名 + 服务清单，与启动时用的 overlay 片无关。
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT="superpowers-obs"
BASE="docker-compose-observability"

cd "$DIR"
echo "[stop:$PROJECT] down..."
docker compose -p "$PROJECT" -f "$BASE.yml" -f "$BASE.prod.yml" down "$@"
