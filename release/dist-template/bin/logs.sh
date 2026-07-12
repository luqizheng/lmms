#!/usr/bin/env bash
# logs.sh — tail 实时日志，参数：服务名（默认 all）
SVC="${1:-}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"
if [[ -n "${SVC}" ]]; then
  docker compose --env-file .env logs -f --tail=100 "${SVC}"
else
  docker compose --env-file .env logs -f --tail=50
fi