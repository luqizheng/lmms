#!/usr/bin/env bash
# verify.sh — 端到端冒烟测试（检查所有服务 HTTP 端口响应）
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"
source "${ROOT}/.env" 2>/dev/null || true

OK=0 FAIL=0
check() {
  local name="$1" url="$2"
  if curl -sfS "${url}" >/dev/null 2>&1; then
    echo "  ✓ ${name}"; ((OK++))
  else
    echo "  ✗ ${name} (${url})"; ((FAIL++))
  fi
}

echo "=== verify: HTTP 端点检查 ==="
check "api-gateway"   "http://localhost:8080/health"
check "config-service" "http://localhost:8081/health"
check "file-service"   "http://localhost:8082/health"
check "token-service"  "http://localhost:8083/health"
check "iam-service"    "http://localhost:8084/health"
check "audit-service"  "http://localhost:8085/health"
check "frontend"       "http://localhost:3000/"
check "prometheus"     "http://localhost:9090/-/healthy"
check "grafana"        "http://localhost:3001/api/health"

echo ""
echo "=== verify: Docker 容器状态 ==="
docker compose --env-file .env ps --format "table {{.Name}}\t{{.Status}}" | grep -v "NAME"

echo ""
echo "结果: ${OK} 通过, ${FAIL} 失败"
[[ "${FAIL}" -eq 0 ]] && exit 0 || exit 1