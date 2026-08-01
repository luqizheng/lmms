#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# scripts/deploy.sh — 从分发包提取纯部署子包
# 在分发包根目录执行，输出：
#   release/coder-micro-services-${VERSION}-deploy.tar.gz
#   release/coder-micro-services-${VERSION}-deploy/
#
# 与 release.sh 的区别：
#   release.sh    → 完整分发包（源码 + 镜像 + 开发骨架），面向二次开发
#   deploy.sh     → 纯部署子包（compose + config + 镜像），面向直接部署
#
# 用法：
#   bash scripts/deploy.sh [VERSION] [--help]
#
# 参数：
#   VERSION              第一个位置参数；默认从 .env 读取 IMAGE_TAG，兜底 v1.0.0
#   --help, -h           打印帮助
#
# 部署子包仅包含：
#   - infrastructure/   docker-compose*.yml + config/ + mysql-init/ + vendor/ + nginx/
#   - images/           预构建的 Docker 镜像 tar
#   - bin/              运维脚本（load-images / start-deploy / stop / status / verify / logs / generate-secrets）
#   - .env.example      环境变量模板
#   - Makefile          运维命令入口（无 build-frontend 目标）
#   - docs/             部署文档
#   - README.md         使用说明
#   - secrets/          密钥目录（空，由 make secrets 填充）
#
# 部署子包不包含（二次开发才需要的）：
#   - frontend/         前端源码（已预构建在镜像中）
#   - services/         服务骨架
#   - contracts/        契约层
#   - scripts/          CI 门禁脚本
#   - skills/ (.agents/) AI 开发 skill
#   - .cargo/           Rust 工具链配置
#   - Cargo.toml        Rust workspace
#   - pnpm-workspace.yaml / package.json
#   - AGENTS.md
#   - CHANGELOG.md
#   - index.html
# ═══════════════════════════════════════════════════════════════════════════════
set -euo pipefail

# ─── args ──────────────────────────────────────────────────────────────────
VERSION=""
SHOW_HELP=0

for arg in "$@"; do
  case "${arg}" in
    --help|-h)  SHOW_HELP=1 ;;
    -*)         echo "[deploy] 未知参数: ${arg}" >&2; SHOW_HELP=1 ;;
    *)          if [[ -z "${VERSION}" ]]; then VERSION="${arg}"; else
                  echo "[deploy] 重复的 VERSION 参数: ${arg}" >&2; SHOW_HELP=1; fi ;;
  esac
done

if [[ "${SHOW_HELP}" == "1" ]]; then
  sed -n '2,50p' "$0" | sed 's/^# \{0,1\}//'
  exit 0
fi

# ─── paths ─────────────────────────────────────────────────────────────────
ROOT="$(cd "$(dirname "$0")/.." && pwd)"          # release/dist-template/ (分发包根目录)

# ─── VERSION detection ─────────────────────────────────────────────────────
if [[ -z "${VERSION}" ]]; then
  # 优先从 .env 读取 IMAGE_TAG
  if [[ -f "${ROOT}/.env" ]]; then
    VERSION=$(grep -E '^IMAGE_TAG=' "${ROOT}/.env" 2>/dev/null | cut -d= -f2 || true)
  fi
  VERSION="${VERSION:-v1.0.0}"
fi

# ─── REGISTRY detection ────────────────────────────────────────────────────
REGISTRY="zhcoder-docker-registry.com:8000"
if [[ -f "${ROOT}/.env" ]]; then
  REGISTRY=$(grep -E '^REGISTRY=' "${ROOT}/.env" 2>/dev/null | cut -d= -f2 || echo "${REGISTRY}")
fi

DIST_DIR="${ROOT}/release/coder-micro-services-${VERSION}-deploy"
DIST_ARCHIVE="${ROOT}/release/coder-micro-services-${VERSION}-deploy.tar.gz"
IMAGES_DIR="${ROOT}/images"

# ─── prerequisite check ────────────────────────────────────────────────────
missing=0
for dep in tar sed; do
  if ! command -v "${dep}" >/dev/null 2>&1; then
    echo "[deploy] ✗ missing prerequisite: ${dep}" >&2
    missing=1
  fi
done
if [[ "${missing}" != "0" ]]; then
  echo "[deploy] fix the above and re-run" >&2
  exit 1
fi

# GNU sed check (same as release.sh)
if ! sed --version >/dev/null 2>&1; then
  echo "[deploy] ✗ GNU sed required (BSD sed detected). On macOS: brew install gnu-sed && alias sed=gsed" >&2
  exit 1
fi

# ─── source validation ─────────────────────────────────────────────────────
# deploy.sh 必须从分发包内运行（ROOT 下应有 infrastructure/ 和 images/）
if [[ ! -d "${ROOT}/infrastructure" ]]; then
  echo "[deploy] ✗ infrastructure/ 目录不存在 — 请在分发包根目录执行此脚本" >&2
  exit 1
fi
if [[ ! -d "${IMAGES_DIR}" ]]; then
  echo "[deploy] ✗ images/ 目录不存在 — 部署包子包需要镜像文件" >&2
  exit 1
fi
if [[ ! -f "${IMAGES_DIR}/manifest.json" ]]; then
  echo "[deploy] ✗ images/manifest.json 不存在" >&2
  exit 1
fi

echo "[deploy] VERSION=${VERSION} REGISTRY=${REGISTRY}"
echo "[deploy] source: ${ROOT}"

# ─── clean & prepare DIST ──────────────────────────────────────────────────
if [[ -d "${DIST_DIR}" ]]; then
  echo "[deploy] wiping stale ${DIST_DIR}"
  rm -rf "${DIST_DIR}"
fi
mkdir -p "${DIST_DIR}"
mkdir -p "${DIST_DIR}/infrastructure"
mkdir -p "${DIST_DIR}/images"
mkdir -p "${DIST_DIR}/bin"
mkdir -p "${DIST_DIR}/secrets"
touch "${DIST_DIR}/secrets/.gitkeep"

# ═══════════════════════════════════════════════════════════════════════════════
# infrastructure/ — Docker Compose 运行时配置
# ═══════════════════════════════════════════════════════════════════════════════
echo "[deploy] -> 复制 infrastructure/ (compose + config)..."

# ── compose 文件 ───────────────────────────────────────────────────────────
DOCKER_COMPOSE_MAIN_SRC="${ROOT}/infrastructure/docker-compose.yml"

if [[ ! -f "${DOCKER_COMPOSE_MAIN_SRC}" ]]; then
  echo "[deploy]   ✗ docker-compose.yml 不存在" >&2
  exit 1
fi

# 复制主 compose 文件，然后替换 frontend 的 build: 块为 image: 引用
cp "${DOCKER_COMPOSE_MAIN_SRC}" "${DIST_DIR}/infrastructure/docker-compose.yml"

# ── frontend build → image 替换 ────────────────────────────────────────────
# 主 docker-compose.yml 中 frontend 服务仍有 build: 块（release.sh 的
# transform-compose.js 只处理了 docker-compose-services.yml）。部署包需要
# 纯 image: 引用，因为不包含 frontend 源码。
transform_frontend_build() {
  local file="$1" registry="$2" version="$3"
  local tmp="${file}.tmp"
  local skip_build=false   # true=正在跳过 build 块内容（6+ 空格缩进的行）

  while IFS= read -r line; do
    # ── 跳过 build 块内的参数行（6+ 空格缩进）──
    if [[ "${skip_build}" == true ]]; then
      if [[ "${line}" =~ ^[[:space:]]{6,} ]]; then
        continue
      fi
      skip_build=false
      # 落在非缩进行上 → 正常输出它
    fi

    # ── 遇到  frontend: → 偷看下一行是否为  build: ──
    if [[ "${line}" == "  frontend:" ]]; then
      echo "${line}"
      # 偷看下一行：单个 read 与外层共享 fd，读到的行外循环不会再读
      if IFS= read -r next_line; then
        if [[ "${next_line}" == "    build:" ]]; then
          echo "    image: ${registry}/micro-server/frontend:${version}"
          skip_build=true
          continue
        else
          echo "${next_line}"
        fi
      fi
      continue
    fi

    echo "${line}"
  done < "${file}" > "${tmp}"

  mv "${tmp}" "${file}"
}

transform_frontend_build \
  "${DIST_DIR}/infrastructure/docker-compose.yml" \
  "${REGISTRY}" "${VERSION}"

echo "[deploy]   ✓ docker-compose.yml: frontend build → image: ${REGISTRY}/micro-server/frontend:${VERSION}"

# ── 其余 compose 文件 ──────────────────────────────────────────────────────
for f in \
    docker-compose-infra.yml \
    docker-compose-infra.dev.yml \
    docker-compose-services.yml \
    docker-compose-observability.yml; do
  if [[ -f "${ROOT}/infrastructure/${f}" ]]; then
    cp "${ROOT}/infrastructure/${f}" "${DIST_DIR}/infrastructure/${f}"
    echo "[deploy]   ✓ infrastructure/${f}"
  fi
done

# ── infrastructure 子目录 ───────────────────────────────────────────────────
for d in config mysql-init nginx vendor; do
  if [[ -d "${ROOT}/infrastructure/${d}" ]]; then
    cp -a "${ROOT}/infrastructure/${d}" "${DIST_DIR}/infrastructure/${d}"
    echo "[deploy]   ✓ infrastructure/${d}/"
  fi
done

# ═══════════════════════════════════════════════════════════════════════════════
# 镜像同步 — 扫描 compose 中所有 image: 引用，保存本地缺失的镜像
# ═══════════════════════════════════════════════════════════════════════════════
echo "[deploy] -> 同步镜像（扫描 compose 中的 image: 引用）..."

sync_images_from_compose() {
  local infra_dir="$1" images_dir="$2"

  # 从 DIST_DIR 下所有 compose 文件中提取 image: 引用
  # 过滤掉空行、注释行、变量引用（如 ${REGISTRY}...）
  local images=()
  while IFS= read -r raw; do
    raw="${raw#"${raw%%[![:space:]]*}"}"    # ltrim
    raw="${raw%"${raw##*[![:space:]]}"}"    # rtrim
    raw="${raw//\"/}"                        # strip quotes
    raw="${raw//\'/}"
    [[ -z "${raw}" ]] && continue
    [[ "${raw}" == \$\{* ]] && continue      # skip unresolved ${VAR} refs
    [[ "${raw}" == "#"* ]] && continue
    images+=("${raw}")
  done < <(
    grep -hE '^[[:space:]]+image:' "${infra_dir}"/docker-compose*.yml 2>/dev/null \
      | sed -E 's/^[[:space:]]+image:[[:space:]]*//'
  )

  # 去重
  local uniq=()
  local seen=""
  for img in "${images[@]}"; do
    if [[ "${seen}" != *"|${img}|"* ]]; then
      uniq+=("${img}")
      seen="${seen}|${img}|"
    fi
  done

  local saved=0 skipped=0 missing=0
  for img in "${uniq[@]}"; do
    local fname
    fname=$(echo "${img}" | tr '/:' '__')

    if [[ -f "${images_dir}/${fname}.tar" ]]; then
      ((skipped++)) || true
      continue
    fi

    if docker image inspect "${img}" >/dev/null 2>&1; then
      echo "[deploy]   save ${img} → ${fname}.tar"
      if docker save "${img}" -o "${images_dir}/${fname}.tar" 2>/dev/null; then
        ((saved++)) || true
      else
        echo "[deploy]   ✗ save 失败: ${img}" >&2
        ((missing++)) || true
      fi
    else
      echo "[deploy]   ⚠ ${img} 不在本地 Docker 中，跳过"
      ((missing++)) || true
    fi
  done

  echo "[deploy]   ✓ 镜像同步: ${saved} 个新增, ${skipped} 个已存在, ${missing} 个缺失"
}

sync_images_from_compose "${DIST_DIR}/infrastructure" "${IMAGES_DIR}"

# ═══════════════════════════════════════════════════════════════════════════════
# images/ — 打包镜像到 DIST
# ═══════════════════════════════════════════════════════════════════════════════
echo "[deploy] -> 复制 images/ (Docker 镜像 tar)..."
if [[ -f "${IMAGES_DIR}/manifest.json" ]]; then
  cp "${IMAGES_DIR}/manifest.json" "${DIST_DIR}/images/manifest.json"
fi
tar_count=0
for tar_file in "${IMAGES_DIR}"/*.tar; do
  [[ -e "${tar_file}" ]] || continue
  fname=$(basename "${tar_file}")
  cp "${tar_file}" "${DIST_DIR}/images/${fname}"
  ((tar_count++)) || true
done
echo "[deploy]   ✓ ${tar_count} 个镜像 tar"

# ═══════════════════════════════════════════════════════════════════════════════
# bin/ — 运维脚本（精简版，不包含 build-frontend / seed-demo / upgrade 等）
# ═══════════════════════════════════════════════════════════════════════════════
echo "[deploy] -> 复制 bin/ (运维脚本)..."
# 部署包需要的脚本：密钥生成 / 镜像加载 / 启动 / 停止 / 状态 / 日志 / 验证
DEPLOY_BIN_SCRIPTS=(
  generate-secrets.sh
  load-images.sh
  start-deploy.sh
  stop.sh
  status.sh
  logs.sh
  verify.sh
)

copied_bin=0
for script in "${DEPLOY_BIN_SCRIPTS[@]}"; do
  if [[ -f "${ROOT}/bin/${script}" ]]; then
    cp "${ROOT}/bin/${script}" "${DIST_DIR}/bin/${script}"
    ((copied_bin++)) || true
  else
    echo "[deploy]   ⚠ bin/${script} 不存在，跳过" >&2
  fi
done
echo "[deploy]   ✓ ${copied_bin} 个运维脚本"

# ═══════════════════════════════════════════════════════════════════════════════
# 顶层文件
# ═══════════════════════════════════════════════════════════════════════════════
echo "[deploy] -> 复制顶层文件..."

# .env.example
if [[ -f "${ROOT}/.env.example" ]]; then
  cp "${ROOT}/.env.example" "${DIST_DIR}/.env.example"
  echo "[deploy]   ✓ .env.example"
else
  echo "[deploy]   ⚠ .env.example 不存在" >&2
fi

# Makefile (使用部署专用版)
if [[ -f "${ROOT}/Makefile.deploy" ]]; then
  cp "${ROOT}/Makefile.deploy" "${DIST_DIR}/Makefile"
  echo "[deploy]   ✓ Makefile (deploy variant)"
elif [[ -f "${ROOT}/Makefile" ]]; then
  cp "${ROOT}/Makefile" "${DIST_DIR}/Makefile"
  echo "[deploy]   ✓ Makefile (从分发包 Makefile 复制)"
else
  echo "[deploy]   ⚠ Makefile 不存在" >&2
fi

# README.md
if [[ -f "${ROOT}/README.md" ]]; then
  cp "${ROOT}/README.md" "${DIST_DIR}/README.md"
  echo "[deploy]   ✓ README.md"
fi

# docs/
if [[ -d "${ROOT}/docs" ]]; then
  cp -a "${ROOT}/docs" "${DIST_DIR}/docs"
  echo "[deploy]   ✓ docs/"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# 排除清单（显式删除，防止 release.sh 新增内容意外泄漏到部署包）
# ═══════════════════════════════════════════════════════════════════════════════
echo "[deploy] -> 清理开发残留（确保以下不存在于部署包）..."
STALE_PATHS=(
  "frontend"
  "services"
  "contracts"
  "scripts"
  ".agents"
  ".cargo"
  ".trae"
  "Cargo.toml"
  "pnpm-workspace.yaml"
  "package.json"
  "AGENTS.md"
  "CHANGELOG.md"
  "index.html"
  "Makefile.deploy"
)
for p in "${STALE_PATHS[@]}"; do
  if [[ -e "${DIST_DIR}/${p}" ]]; then
    rm -rf "${DIST_DIR}/${p}"
    echo "[deploy]   ✗ removed stale: ${p}"
  fi
done
# 同时清理 bin/ 下不属于部署包的脚本
for f in "${DIST_DIR}/bin"/*; do
  [[ -f "${f}" ]] || continue
  fname=$(basename "${f}")
  allowed=false
  for allowed_name in "${DEPLOY_BIN_SCRIPTS[@]}"; do
    [[ "${fname}" == "${allowed_name}" ]] && allowed=true && break
  done
  if [[ "${allowed}" == false ]]; then
    rm -f "${f}"
    echo "[deploy]   ✗ removed stale bin script: ${fname}"
  fi
done

# ═══════════════════════════════════════════════════════════════════════════════
# tar
# ═══════════════════════════════════════════════════════════════════════════════
echo "[deploy] -> 打包 tar.gz..."
(
  cd "${ROOT}/release"
  tar -czf "coder-micro-services-${VERSION}-deploy.tar.gz" \
    "coder-micro-services-${VERSION}-deploy/"
)

echo ""
echo "[deploy] ✓ 完成！"
echo ""
echo "输出文件："
echo "  部署目录: ${DIST_DIR}/"
echo "  部署压缩包: ${DIST_ARCHIVE} ($(du -h "${DIST_ARCHIVE}" | cut -f1))"
echo ""
echo "部署步骤："
echo "  1. 解压：tar -xzf coder-micro-services-${VERSION}-deploy.tar.gz"
echo "  2. 进入：cd coder-micro-services-${VERSION}-deploy"
echo "  3. 加载镜像：make load-images"
echo "  4. 生成密钥：make secrets"
echo "  5. 启动服务：make up"
echo ""
echo "⚠️  本部署包仅包含 compose + config + 镜像，不含开发源码。"
echo "   如需二次开发，请使用完整分发包（coder-micro-services-*-airgap.tar.gz）。"
