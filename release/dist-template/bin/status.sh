#!/usr/bin/env bash
# status.sh — 列出所有容器运行状态
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"
docker compose --env-file .env ps --format "table {{.Name}}\t{{.Status}}\t{{.Ports}}"