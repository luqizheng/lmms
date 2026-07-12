#!/usr/bin/env bash
# load-images.sh — 将离线 tar 镜像包加载到本地 Docker（air-gap 安装）
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMAGES_DIR="${ROOT}/images"

if [[ ! -d "${IMAGES_DIR}" ]]; then
  echo "[load-images] ✗ ${IMAGES_DIR} 不存在，请先放置 .tar 镜像包"
  exit 1
fi

if [[ ! -f "${IMAGES_DIR}/manifest.json" ]]; then
  echo "[load-images] ✗ manifest.json 不存在"
  exit 1
fi

echo "[load-images] 开始加载离线镜像..."
for tar in "${IMAGES_DIR}"/*.tar; do
  [[ -e "${tar}" ]] || continue
  echo "[load-images]   → docker load -i $(basename "${tar}")"
  docker load -i "${tar}"
done
echo "[load-images] ✓ 加载完成"