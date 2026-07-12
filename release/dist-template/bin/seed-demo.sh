#!/usr/bin/env bash
# seed-demo.sh — 向 API 网关写入 demo 数据（用户/角色/权限/项目）
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"
source "${ROOT}/.env" 2>/dev/null || true

GATEWAY="${API_GATEWAY_URL:-http://localhost:8080}"
echo "[seed-demo] → POST /api/v1/seed to ${GATEWAY}"
curl -sfS -X POST "${GATEWAY}/api/v1/seed" \
  -H "Content-Type: application/json" \
  -d '{"environment":"demo"}' \
  && echo "[seed-demo] ✓ demo 数据写入成功" \
  || echo "[seed-demo] ✗ demo 数据写入失败"