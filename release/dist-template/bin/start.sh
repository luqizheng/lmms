#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# start.sh — 启动所有容器（含 healthcheck 等待）
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

# 1. 确保 frontend 镜像存在
bash "${ROOT}/bin/build-frontend.sh"

# 2. 启动所有容器
echo "[start] → docker compose up -d"
docker compose --env-file .env -f infrastructure/docker-compose.yml up -d

# 3. 等待关键服务健康
echo "[start] 等待关键服务健康（最多 120s）..."
for svc in mysql consul redis rabbitmq api-gateway; do
  for i in $(seq 1 60); do
    STATE=$(docker inspect --format='{{.State.Health.Status}}' \
      "coder-micro-services-${svc}" 2>/dev/null || echo "not-found")
    if [[ "${STATE}" == "healthy" ]]; then
      echo "[start]   ✓ ${svc} healthy"
      break
    fi
    if [[ "${STATE}" == "not-found" ]]; then
      echo "[start]   ✗ ${svc} 未找到（容器名可能不匹配）"
      break
    fi
    if [[ $i -eq 60 ]]; then
      echo "[start]   ✗ ${svc} 健康检查超时 (state=${STATE})"
    fi
    sleep 2
  done
done

echo "[start] ✓ 启动完成"
echo "[start] 验证：curl http://localhost:3000/health"
curl -fsS http://localhost:3000/health && echo " → OK" || echo " → 失败（服务可能仍在启动）"