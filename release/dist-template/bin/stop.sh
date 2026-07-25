#!/usr/bin/env bash
# stop.sh — 停止所有容器（保留 volumes）
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"
echo "[stop] → docker compose down"
docker compose --env-file .env -f infrastructure/docker-compose.yml down
echo "[stop] ✓ 停止完成（volumes 已保留）"