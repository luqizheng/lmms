#!/usr/bin/env bash
# stop.sh —— 停止全栈部署单元（infra + observability + services）
# ─────────────────────────────────────────────────────────────────────────────
# 用法：
#   ./stop.sh              # down（保留 named volumes）
#   ./stop.sh --volumes    # 连 named volume 一起删（清库/清监控数据）
#   ./stop.sh --help
#
# 定位依据是 project 名，与启动时用的 overlay 片 / --logs 无关。
# 本单元的 project 名是 superpowers-full（与 docker-deploy 的 superpowers-svc 区分）。
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT="superpowers-full"

cd "$DIR"
echo "[stop:$PROJECT] down..."
docker compose -p "$PROJECT" -f docker-compose.yml down "$@"
