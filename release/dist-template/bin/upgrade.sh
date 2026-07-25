#!/usr/bin/env bash
# upgrade.sh — 保留 volumes 就地升级镜像
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

NEW_TAG="${1:-}"
if [[ -z "${NEW_TAG}" ]]; then
  echo "用法: $0 <新版本号>  例如: $0 v1.1.0"
  exit 1
fi

echo "[upgrade] → 拉取新镜像 tag=${NEW_TAG} ..."
REGISTRY="${REGISTRY:-zhcoder-docker-registry.com:8000}"
docker compose --env-file .env -f infrastructure/docker-compose.yml pull

echo "[upgrade] → 重启服务（volumes 保留）..."
docker compose --env-file .env -f infrastructure/docker-compose.yml up -d

echo "[upgrade] ✓ 升级完成: ${NEW_TAG}"
echo "[upgrade] 验证: bash bin/verify.sh"