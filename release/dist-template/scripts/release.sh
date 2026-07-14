#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# scripts/release.sh - 打包离线分发包
# 在有网络的环境执行，输出：
#   release/coder-micro-services-${VERSION}-airgap.tar.gz
#   release/coder-micro-services-${VERSION}/
#
# 用法：
#   bash scripts/release.sh [VERSION] [--no-images|--images] [--help]
#   EXPORT_IMAGES=0 bash scripts/release.sh [VERSION]
#
# 参数：
#   VERSION              第一个位置参数，默认 v1.0.0
#   --no-images          跳过 docker pull + docker save（仅打文档/配置包）
#   --images             显式导出镜像（默认行为，与省略等价）
#   --help, -h           打印帮助
#
# 环境变量：
#   REGISTRY             镜像仓库，默认 zhcoder-docker-registry.com:8000
#   EXPORT_IMAGES        1=导出镜像（默认），0=跳过；被 --no-images/--images 覆盖
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

VERSION=""
EXPORT_IMAGES="${EXPORT_IMAGES:-1}"
SHOW_HELP=0

for arg in "$@"; do
  case "${arg}" in
    --no-images)  EXPORT_IMAGES=0 ;;
    --images)     EXPORT_IMAGES=1 ;;
    --help|-h)    SHOW_HELP=1 ;;
    -*)           echo "[release] 未知参数: ${arg}" >&2; SHOW_HELP=1 ;;
    *)            if [[ -z "${VERSION}" ]]; then VERSION="${arg}"; else
                    echo "[release] 重复的 VERSION 参数: ${arg}" >&2; SHOW_HELP=1; fi ;;
  esac
done

if [[ "${SHOW_HELP}" == "1" ]]; then
  sed -n '2,21p' "$0" | sed 's/^# \{0,1\}//'
  exit 0
fi

VERSION="${VERSION:-v1.0.0}"
REGISTRY="${REGISTRY:-zhcoder-docker-registry.com:8000}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST_DIR="${ROOT}/release/coder-micro-services-${VERSION}"
DIST_ARCHIVE="${ROOT}/release/coder-micro-services-${VERSION}-airgap.tar.gz"
IMAGES_DIR="${DIST_DIR}/images"

# DIST_DIR 必须存在（否则 --no-images + VitePress 跳过时 tar 会失败）；
# IMAGES_DIR 仅在导出镜像时创建，避免空 images/ 目录混入分发包。
mkdir -p "${DIST_DIR}"
if [[ "${EXPORT_IMAGES}" == "1" ]]; then
  mkdir -p "${IMAGES_DIR}"
fi

echo "[release] VERSION=${VERSION} REGISTRY=${REGISTRY} EXPORT_IMAGES=${EXPORT_IMAGES}"

# 镜像清单
IMAGES=(
  "${REGISTRY}/micro-server/api-gateway:${VERSION}"
  "${REGISTRY}/micro-server/config-service:${VERSION}"
  "${REGISTRY}/micro-server/file-service:${VERSION}"
  "${REGISTRY}/micro-server/token-service:${VERSION}"
  "${REGISTRY}/micro-server/iam-service:${VERSION}"
  "${REGISTRY}/micro-server/notification-service:${VERSION}"
  "${REGISTRY}/micro-server/audit-service:${VERSION}"
  "${REGISTRY}/micro-server/prue-backend:${VERSION}"
  "${REGISTRY}/micro-server/frontend:${VERSION}"
  "hashicorp/consul:2.0.1"
  "gliderlabs/registrator:master"
  "redis:7-alpine"
  "mysql:8.0"
  "rabbitmq:3.13-management-alpine"
  "prom/prometheus:v2.51.0"
  "grafana/grafana:10.4.0"
  "hashicorp/consul-template:0.32.0"
  "otel/opentelemetry-collector-contrib:0.96.0"
  "jaegertracing/all-in-one:1.55"
)

if [[ "${EXPORT_IMAGES}" == "1" ]]; then
  echo "[release] -> 拉取所有镜像..."
  for img in "${IMAGES[@]}"; do
    echo "[release]   pull ${img}"
    docker pull "${img}" 2>/dev/null || echo "[release]   ✗ pull failed: ${img}"
  done

  echo "[release] -> 导出所有镜像为 tar..."
  for img in "${IMAGES[@]}"; do
    fname=$(echo "${img}" | tr '/:' '__')
    echo "[release]   save ${img} -> ${fname}.tar"
    docker save "${img}" -o "${IMAGES_DIR}/${fname}.tar" 2>/dev/null && \
      echo "[release]   ✓ ${fname}.tar" || \
      echo "[release]   ✗ save failed: ${img}"
  done
else
  echo "[release] ⚠ --no-images：跳过 docker pull/save"
fi

echo "[release] -> 构建 VitePress 文档..."
if command -v npx >/dev/null 2>&1; then
  DOCS_SRC="${DIST_DIR}/docs"
  if [[ -f "${DOCS_SRC}/.vitepress/config.ts" ]]; then
    echo "[release]   npx vitepress build ${DOCS_SRC}"
    cd "${DOCS_SRC}"
    npx --yes vitepress@latest build . 2>/dev/null && \
      cp -r "${DOCS_SRC}/.vitepress/dist/"* "${DIST_DIR}/" && \
      echo "[release]   ✓ VitePress 文档已构建到根目录" || \
      echo "[release]   ⚠ vitepress build failed，保留 index.html 占位符"
    cd "${ROOT}"
  else
    echo "[release]   ⚠ docs/.vitepress/config.ts 不存在，跳过 VitePress 构建"
  fi
else
  echo "[release]   ⚠ npx 不可用，保留 index.html 占位符"
fi

echo "[release] -> 打包 tar.gz..."
cd "${ROOT}/release"
tar -czf "coder-micro-services-${VERSION}-airgap.tar.gz" \
  "coder-micro-services-${VERSION}/"

echo "[release] ✓ 完成！"
echo ""
echo "输出文件："
ls -lh "${DIST_DIR}/"
ls -lh "${DIST_ARCHIVE}"
echo ""
if [[ "${EXPORT_IMAGES}" == "1" ]]; then
  echo "离线安装步骤："
  echo "  1. 解压：tar -xzf coder-micro-services-${VERSION}-airgap.tar.gz"
  echo "  2. 加载镜像：bash bin/load-images.sh"
  echo "  3. 初始化：make secrets"
  echo "  4. 启动：make up"
  echo ""
  echo "文档：直接打开 index.html（浏览器 file:// 协议即可）"
else
  echo "⚠ 本次未导出镜像，离线安装需先自行 docker pull 或单独执行："
  echo "    EXPORT_IMAGES=1 bash scripts/release.sh ${VERSION}"
  echo "文档：直接打开 index.html（浏览器 file:// 协议即可）"
fi
