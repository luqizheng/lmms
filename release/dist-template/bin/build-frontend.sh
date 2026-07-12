#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# build-frontend.sh — 从 ./frontend 源码构建 nginx 镜像
# BuildKit cache mount 命中后增量秒级；镜像存在则跳过
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

REGISTRY="${REGISTRY:-zhcoder-docker-registry.com:8000}"
TAG="${IMAGE_TAG:-$(grep -E '^IMAGE_TAG=' .env 2>/dev/null | cut -d= -f2 || echo v1.0.0)}"
IMG="${REGISTRY}/micro-server/frontend:${TAG}"

if docker image inspect "${IMG}" >/dev/null 2>&1; then
  echo "[build-frontend] ✓ ${IMG} 已存在，跳过 build"
  exit 0
fi

echo "[build-frontend] → 构建 ${IMG}"
echo "[build-frontend]   首次 build 约需 1-2 分钟（pnpm install），后续命中 cache 秒级"

DOCKER_BUILDKIT=1 docker build \
  --build-arg BUILDKIT_INLINE_CACHE=1 \
  --build-arg VITE_API_BASE="${VITE_API_BASE:-/api}" \
  --build-arg VITE_PM_API_BASE="${VITE_PM_API_BASE:-/pm-api}" \
  --build-arg VITE_DISABLE_CAPTCHA="${VITE_DISABLE_CAPTCHA:-true}" \
  --tag "${IMG}" \
  ./frontend

echo "[build-frontend] ✓ 构建完成: ${IMG}"

if [[ "${PUSH:-0}" == "1" ]]; then
  echo "[build-frontend] → push ${IMG}"
  docker push "${IMG}"
fi