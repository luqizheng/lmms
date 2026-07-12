#!/usr/bin/env bash
# reset-data.sh — 清除所有数据 volumes 并重新初始化 MySQL init scripts
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"
echo "[reset-data] → 停止所有容器并清除数据卷..."
docker compose --env-file .env down -v
echo "[reset-data] ✓ 数据卷已清除"
echo "[reset-data] → 重新启动..."
bash "${ROOT}/bin/start.sh"