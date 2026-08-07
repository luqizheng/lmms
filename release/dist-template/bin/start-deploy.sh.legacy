#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# start-deploy.sh — 启动所有容器（纯部署模式，无 frontend 构建步骤）
# 适用于 deploy.sh 产出的部署包：镜像已预构建并加载，无需本地构建
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

echo "[start] → docker compose up -d"
docker compose --env-file .env -f infrastructure/docker-compose.yml up -d

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
