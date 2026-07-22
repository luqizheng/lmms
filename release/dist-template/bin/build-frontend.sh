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

# 选择构建上下文：
# - CI 环境（dev 仓库 checkout）：优先用 ../../frontend（最新源码，
#   避免 dist-template/frontend/ 快照漂移导致推出过期前端镜像）
# - 分发包用户（解压 tar.gz 后）：用 ./frontend（release.sh 复制过来的兜底源码）
if [ -d "${ROOT}/../../frontend" ]; then
  BUILD_CONTEXT="${ROOT}/../../frontend"
  echo "[build-frontend] -> 使用 dev 仓库 frontend/ 作为构建上下文: ${BUILD_CONTEXT}"
else
  BUILD_CONTEXT="${ROOT}/frontend"
  echo "[build-frontend] -> 使用分发包 frontend/ 作为构建上下文: ${BUILD_CONTEXT}"
fi

DOCKER_BUILDKIT=1 docker build \
  --build-arg BUILDKIT_INLINE_CACHE=1 \
  --build-arg VITE_API_BASE="${VITE_API_BASE:-/api}" \
  --build-arg VITE_DISABLE_CAPTCHA="${VITE_DISABLE_CAPTCHA:-true}" \
  --tag "${IMG}" \
  "${BUILD_CONTEXT}"

echo "[build-frontend] ✓ 构建完成: ${IMG}"

if [[ "${PUSH:-0}" == "1" ]]; then
  echo "[build-frontend] → push ${IMG}"
  docker push "${IMG}"
fi