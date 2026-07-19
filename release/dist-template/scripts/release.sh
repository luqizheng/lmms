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
#   --no-images          跳过 docker pull + docker save（仅打配置/骨架包）
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

# DIST_DIR 必须存在（tar 源目录不能为空）；
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

echo "[release] -> 跳过文档构建（user-manual 归 docs-site/ 管理，不打包进分发包）..."
echo ""

echo "[release] -> 同步开发骨架（libs/sdk/contracts/skills/scripts/services）..."

# 开发仓库根（dist-template 的上两级）
REPO_ROOT="$(cd "${ROOT}/../.." && pwd)"

# Rust 共享库（业务 Rust 服务编译时依赖 path = "../../libs/..."）
if [ -d "${REPO_ROOT}/libs" ]; then
  cp -r "${REPO_ROOT}/libs" "${DIST_DIR}/libs"
  find "${DIST_DIR}/libs" -type d -name 'target' -exec rm -rf {} + 2>/dev/null || true
  echo "[release]   ✓ libs/"
else
  echo "[release]   ⚠ libs/ not found at ${REPO_ROOT}/libs - Rust 业务服务将无法编译"
fi

# SDK 目录：完全不打进分发包。
# sdk/java/* — 由 CI sdk_publish job 推送到私有 Maven 仓库
#               (http://192.168.2.12:18181/releases，由 MAVEN_REPO_URL 覆盖)，
#               业务 Java 服务通过 <dependency> 直接引入。
# sdk/rust/* — 以 crate 形式发布到 crates.io（或内网 Panamax mirror
#               http://192.168.2.12:16444），业务 Rust 服务通过 `cargo add` 引入。
# sdk/openapi + sdk/scripts + sdk/README — 开发者仓库内部资产，
#               分发包用户通过包管理器拿到 SDK，无需源码或本地发布脚本。
#               根 Cargo.toml 的 workspace members 仍列出 sdk/rust/*，
#               下文会用 sed 过滤掉，避免指向不存在的目录。
echo "[release]   ⚠ sdk/ skipped — entire sdk/ published to private package registries"

# 契约层（前后端单一真相源：error-codes / permissions / events / conventions）
if [ -d "${REPO_ROOT}/contracts" ]; then
  cp -r "${REPO_ROOT}/contracts" "${DIST_DIR}/contracts"
  echo "[release]   ✓ contracts/"
fi

# CI 门禁脚本（check-service-standard / check-db-naming / check-spec / contracts-gen 等）
if [ -d "${REPO_ROOT}/scripts" ]; then
  cp -r "${REPO_ROOT}/scripts" "${DIST_DIR}/scripts"
  echo "[release]   ✓ scripts/"
fi

# AI skills（复制到 .agents/skills/ 以符合 agent 发现规范）
if [ -d "${REPO_ROOT}/.agents/skills" ]; then
  mkdir -p "${DIST_DIR}/.agents"
  cp -r "${REPO_ROOT}/.agents/skills" "${DIST_DIR}/.agents/skills"
  echo "[release]   ✓ .agents/skills/"
fi

# AI 规则文件（.agents/rules/ — 项目级 L4 触发规则，替换原 .trae/rules/ 约定）
if [ -d "${REPO_ROOT}/.agents/rules" ]; then
  mkdir -p "${DIST_DIR}/.agents"
  cp -r "${REPO_ROOT}/.agents/rules" "${DIST_DIR}/.agents/rules"
  echo "[release]   ✓ .agents/rules/"
elif [ -d "${REPO_ROOT}/.trae/rules" ]; then
  # 兼容旧版：从 .trae/rules/ 迁移到 .agents/rules/
  mkdir -p "${DIST_DIR}/.agents/rules"
  cp -r "${REPO_ROOT}/.trae/rules/"* "${DIST_DIR}/.agents/rules/"
  echo "[release]   ✓ .agents/rules/ (migrated from .trae/rules/)"
fi

# 前端源码（compose/services.yml 的 frontend 服务 build context: ./frontend，
# 离线镜像缺失时供 docker build 兜底）
if [ -d "${REPO_ROOT}/frontend" ]; then
  cp -r "${REPO_ROOT}/frontend" "${DIST_DIR}/frontend"
  find "${DIST_DIR}/frontend" -type d \( -name 'node_modules' -o -name 'dist' -o -name 'dist-ssr' -o -name 'coverage' \) -exec rm -rf {} + 2>/dev/null || true
  find "${DIST_DIR}/frontend" -type f \( -name '.eslintcache' -o -name '*.local' -o -name 'report.html' \) -delete 2>/dev/null || true
  echo "[release]   ✓ frontend/ (node_modules / dist / dist-ssr / coverage / lint-cache excluded)"
else
  echo "[release]   ⚠ frontend/ not found at ${REPO_ROOT}/frontend - 前端容器镜像缺失时将无法本地构建"
fi

# services/ 目录骨架（空 - 用户通过 skills 创建业务微服务）
mkdir -p "${DIST_DIR}/services"
if [ -f "${REPO_ROOT}/services/agents-template.md" ]; then
  cp "${REPO_ROOT}/services/agents-template.md" "${DIST_DIR}/services/agents-template.md"
fi
touch "${DIST_DIR}/services/.gitkeep"
echo "[release]   ✓ services/ (empty - for user-created business services)"

# infrastructure/ 目录（Docker Compose + 配置，发布后即刻启动开发环境）
if [ -d "${REPO_ROOT}/infrastructure" ]; then
  mkdir -p "${DIST_DIR}/infrastructure"

  # 复制所有 compose 文件
  cp "${REPO_ROOT}/infrastructure/docker-compose.yml" "${DIST_DIR}/infrastructure/docker-compose.yml" 2>/dev/null || true
  cp "${REPO_ROOT}/infrastructure/docker-compose-infra.yml" "${DIST_DIR}/infrastructure/docker-compose-infra.yml" 2>/dev/null || true
  cp "${REPO_ROOT}/infrastructure/docker-compose-infra.dev.yml" "${DIST_DIR}/infrastructure/docker-compose-infra.dev.yml" 2>/dev/null || true
  cp "${REPO_ROOT}/infrastructure/docker-compose-services.yml" "${DIST_DIR}/infrastructure/docker-compose-services.yml" 2>/dev/null || true
  cp "${REPO_ROOT}/infrastructure/docker-compose-observability.yml" "${DIST_DIR}/infrastructure/docker-compose-observability.yml" 2>/dev/null || true

  # 转换 docker-compose-services.yml：将 build: 块替换为 image: 引用
  # （分发包用户通过已推送的镜像启动，无需本地构建）
  if [ -f "${DIST_DIR}/infrastructure/docker-compose-services.yml" ]; then
    node "${ROOT}/scripts/transform-compose.js" \
      "${DIST_DIR}/infrastructure/docker-compose-services.yml" \
      "${REGISTRY}" "${VERSION}" && \
      echo "[release]   ✓ docker-compose-services.yml: build → image 转换完成" || \
      echo "[release]   ⚠ docker-compose-services.yml 转换失败（非致命）"
  fi

  echo "[release]   ✓ infrastructure/docker-compose*.yml"

  # 复制配置目录（API 网关路由、Consul、Grafana、Prometheus、OTel 等）
  if [ -d "${REPO_ROOT}/infrastructure/config" ]; then
    cp -r "${REPO_ROOT}/infrastructure/config" "${DIST_DIR}/infrastructure/config"
    echo "[release]   ✓ infrastructure/config/"
  fi

  # 复制 MySQL 初始化脚本
  if [ -d "${REPO_ROOT}/infrastructure/mysql-init" ]; then
    cp -r "${REPO_ROOT}/infrastructure/mysql-init" "${DIST_DIR}/infrastructure/mysql-init"
    echo "[release]   ✓ infrastructure/mysql-init/"
  fi

  # 复制 Nginx 配置
  if [ -d "${REPO_ROOT}/infrastructure/nginx" ]; then
    cp -r "${REPO_ROOT}/infrastructure/nginx" "${DIST_DIR}/infrastructure/nginx"
    echo "[release]   ✓ infrastructure/nginx/"
  fi

  # 复制 vendor（swagger-ui zip，build.rs 需要）
  if [ -d "${REPO_ROOT}/infrastructure/vendor" ]; then
    cp -r "${REPO_ROOT}/infrastructure/vendor" "${DIST_DIR}/infrastructure/vendor"
    echo "[release]   ✓ infrastructure/vendor/"
  fi

  # 复制 .env 模板
  if [ -f "${REPO_ROOT}/infrastructure/.env" ]; then
    cp "${REPO_ROOT}/infrastructure/.env" "${DIST_DIR}/infrastructure/.env"
    echo "[release]   ✓ infrastructure/.env"
  fi

  echo "[release]   ✓ infrastructure/ (Docker Compose + configs — ready for dev environment)"
else
  echo "[release]   ⚠ infrastructure/ not found — 发布后无法直接启动开发环境"
fi

# Rust workspace 根 Cargo.toml（移除 services/* 和 sdk/rust/* members，
# services/ 用户通过 skill 动态加入；sdk/rust/ 以 crate 发布，不打包源码）
if [ -f "${REPO_ROOT}/Cargo.toml" ]; then
  sed '/^[[:space:]]*"services\//d; /^[[:space:]]*"sdk\/rust\//d' "${REPO_ROOT}/Cargo.toml" > "${DIST_DIR}/Cargo.toml"
  echo "[release]   ✓ Cargo.toml (workspace: libs only, services/ + sdk/rust/ excluded)"
fi

# pnpm workspace + package.json（根调度脚本，CI 门禁命令）
if [ -f "${REPO_ROOT}/pnpm-workspace.yaml" ]; then
  cp "${REPO_ROOT}/pnpm-workspace.yaml" "${DIST_DIR}/pnpm-workspace.yaml"
fi
if [ -f "${REPO_ROOT}/package.json" ]; then
  cp "${REPO_ROOT}/package.json" "${DIST_DIR}/package.json"
fi
echo "[release]   ✓ pnpm-workspace.yaml + package.json"

# AGENTS.md（项目协作指南，skill 引用的约定）
if [ -f "${REPO_ROOT}/AGENTS.md" ]; then
  cp "${REPO_ROOT}/AGENTS.md" "${DIST_DIR}/AGENTS.md"
  # 将 .trae/ 引用替换为 .agents/（分发包不使用 Trae IDE）
  sed -i 's|\.trae/rules/|.agents/rules/|g' "${DIST_DIR}/AGENTS.md"
  sed -i 's|\.trae/rule/|.agents/rules/|g' "${DIST_DIR}/AGENTS.md"
  sed -i 's|\.trae/skills/|.agents/skills/|g' "${DIST_DIR}/AGENTS.md"
  # 移除平台 skill 表行（.trae/skills/ 在分发包中不存在）
  sed -i '/| 同步基础镜像 |.*\.trae\/skills\//d' "${DIST_DIR}/AGENTS.md"
  sed -i '/| 审计服务规范 |.*\.trae\/skills\//d' "${DIST_DIR}/AGENTS.md"
  sed -i '/| 引导开发环境 |.*\.trae\/skills\//d' "${DIST_DIR}/AGENTS.md"
  echo "[release]   ✓ AGENTS.md (.trae/ references replaced with .agents/)"
fi

echo "[release] ✓ 开发骨架同步完成"
echo ""

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
  echo "文档：分发包 docs/ 目录内有 Markdown 源可直接查看；"
  echo "      完整开发文档请参见开发仓库 docs-site/ 目录。"
else
echo "⚠ 本次未导出镜像，离线安装需先自行 docker pull 或单独执行："
echo "    EXPORT_IMAGES=1 bash scripts/release.sh ${VERSION}"
  echo "文档：分发包 docs/ 目录内有 Markdown 源可直接查看；"
  echo "      完整开发文档请参见开发仓库 docs-site/ 目录。"
fi
