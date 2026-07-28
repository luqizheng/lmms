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

# ─── args ──────────────────────────────────────────────────────────────────
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

# ─── paths ─────────────────────────────────────────────────────────────────
ROOT="$(cd "$(dirname "$0")/.." && pwd)"     # release/dist-template/
REPO_ROOT="$(cd "${ROOT}/../.." && pwd)"     # repo root
DIST_DIR="${ROOT}/release/coder-micro-services-${VERSION}"
DIST_ARCHIVE="${ROOT}/release/coder-micro-services-${VERSION}-airgap.tar.gz"
IMAGES_DIR="${DIST_DIR}/images"

# ─── prerequisite check ────────────────────────────────────────────────────
# Fail fast when a dependency is missing so partial output does not leak.
# docker is only required when EXPORT_IMAGES=1.
check_dep() {
  local name="$1"
  if ! command -v "${name}" >/dev/null 2>&1; then
    echo "[release] ✗ missing prerequisite: ${name}" >&2
    return 1
  fi
}
missing=0
check_dep node || missing=1
check_dep tar  || missing=1
check_dep sed  || missing=1
if [[ "${EXPORT_IMAGES}" == "1" ]]; then
  check_dep docker || missing=1
fi
if [[ "${missing}" != "0" ]]; then
  echo "[release] fix the above and re-run" >&2
  exit 1
fi

# Our `sed -i 'expr' file` (no backup arg) is GNU-only. macOS BSD sed silently
# does the wrong thing. Detect and reject upfront.
if ! sed --version >/dev/null 2>&1; then
  echo "[release] ✗ GNU sed required (BSD sed detected). On macOS: brew install gnu-sed && alias sed=gsed" >&2
  exit 1
fi

# ─── clean & prepare DIST ──────────────────────────────────────────────────
# Blow away DIST_DIR completely so leftovers from a previous run cannot
# leak into this build (e.g. deleted skills, renamed configs).
if [[ -d "${DIST_DIR}" ]]; then
  echo "[release] wiping stale ${DIST_DIR}"
  rm -rf "${DIST_DIR}"
fi
mkdir -p "${DIST_DIR}"
if [[ "${EXPORT_IMAGES}" == "1" ]]; then
  mkdir -p "${IMAGES_DIR}"
fi

echo "[release] VERSION=${VERSION} REGISTRY=${REGISTRY} EXPORT_IMAGES=${EXPORT_IMAGES}"

# ─── image manifest ────────────────────────────────────────────────────────
IMAGES=(
  "${REGISTRY}/micro-server/api-gateway:${VERSION}"
  "${REGISTRY}/micro-server/config-service:${VERSION}"
  "${REGISTRY}/micro-server/file-service:${VERSION}"
  "${REGISTRY}/micro-server/token-service:${VERSION}"
  "${REGISTRY}/micro-server/iam-service:${VERSION}"
  "${REGISTRY}/micro-server/notification-service:${VERSION}"
  "${REGISTRY}/micro-server/audit-service:${VERSION}"
  "${REGISTRY}/micro-server/prue-admin-backend:${VERSION}"
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
    if docker save "${img}" -o "${IMAGES_DIR}/${fname}.tar" 2>/dev/null; then
      echo "[release]   ✓ ${fname}.tar"
    else
      echo "[release]   ✗ save failed: ${img}"
    fi
  done
else
  echo "[release] ⚠ --no-images：跳过 docker pull/save"
fi

# ─── skeleton from release/dist-template/ ──────────────────────────────────
# The dist-template dir carries the pre-baked runtime scaffolding (Makefile,
# bin/*, README, index.html, docs/, CHANGELOG, .env.example). Copy it wholesale
# minus this scripts/ dir and any prior build output under release/.
echo "[release] -> 同步分发包运行时骨架（dist-template/{Makefile,bin,docs,README,...}）..."
(
  cd "${ROOT}"
  find . -mindepth 1 -maxdepth 1 \
    \! -name 'scripts' \
    \! -name 'release' \
    \! -name 'images' \
    \! -name 'secrets' \
    -print0 |
    while IFS= read -r -d '' item; do
      cp -a "${item}" "${DIST_DIR}/"
    done
)
mkdir -p "${DIST_DIR}/secrets"
touch "${DIST_DIR}/secrets/.gitkeep"
echo "[release]   ✓ dist-template skeleton copied"

# ─── dev repo artifacts ────────────────────────────────────────────────────
echo "[release] -> 同步开发骨架（contracts/scripts/skills/frontend/services/infrastructure）..."

# libs/* and sdk/rust/* are published to the private kellnr registry, sdk/java
# to Maven; users pull them via `cargo add --registry kellnr` and Maven deps.
# Ship neither the source nor the workspace member declarations (sed below).
echo "[release]   ⚠ libs/ skipped - 17 crates published to private kellnr registry"
echo "[release]   ⚠ sdk/ skipped — sdk/java -> Maven, sdk/rust -> kellnr"

if [ -d "${REPO_ROOT}/contracts" ]; then
  cp -a "${REPO_ROOT}/contracts" "${DIST_DIR}/contracts"
  echo "[release]   ✓ contracts/"
fi

if [ -d "${REPO_ROOT}/scripts" ]; then
  cp -a "${REPO_ROOT}/scripts" "${DIST_DIR}/scripts"
  echo "[release]   ✓ scripts/"
fi

# tests/e2e ships so dist users can run both API (vitest) and browser (Playwright)
# suites. Runtime artifacts are excluded; users reinstall + regenerate locally:
#   pnpm -C tests/e2e install && npx playwright install chromium
if [ -d "${REPO_ROOT}/tests/e2e" ]; then
  mkdir -p "${DIST_DIR}/tests"
  cp -a "${REPO_ROOT}/tests/e2e" "${DIST_DIR}/tests/e2e"
  find "${DIST_DIR}/tests/e2e" -type d \
    \( -name 'node_modules' -o -name 'test-results' -o -name 'playwright-report' \) \
    -exec rm -rf {} + 2>/dev/null || true
  rm -f "${DIST_DIR}/tests/e2e/shared/report/"report-*.xlsx 2>/dev/null || true
  rm -f "${DIST_DIR}/tests/e2e/ui/dev/last-error.png" 2>/dev/null || true
  find "${DIST_DIR}/tests/e2e/shared/report/errors" -mindepth 1 -delete 2>/dev/null || true
  echo "[release]   ✓ tests/e2e/ (node_modules / reports / playwright artifacts excluded)"
fi

if [ -d "${REPO_ROOT}/.agents/skills" ]; then
  mkdir -p "${DIST_DIR}/.agents"
  cp -a "${REPO_ROOT}/.agents/skills" "${DIST_DIR}/.agents/skills"
  echo "[release]   ✓ .agents/skills/"
fi

if [ -d "${REPO_ROOT}/.agents/rules" ]; then
  mkdir -p "${DIST_DIR}/.agents"
  cp -a "${REPO_ROOT}/.agents/rules" "${DIST_DIR}/.agents/rules"
  echo "[release]   ✓ .agents/rules/"
elif [ -d "${REPO_ROOT}/.trae/rules" ]; then
  mkdir -p "${DIST_DIR}/.agents/rules"
  cp -a "${REPO_ROOT}/.trae/rules/." "${DIST_DIR}/.agents/rules/"
  echo "[release]   ✓ .agents/rules/ (migrated from .trae/rules/)"
fi

if [ -d "${REPO_ROOT}/frontend" ]; then
  cp -a "${REPO_ROOT}/frontend" "${DIST_DIR}/frontend"
  find "${DIST_DIR}/frontend" -type d \( -name 'node_modules' -o -name 'dist' -o -name 'dist-ssr' -o -name 'coverage' \) -exec rm -rf {} + 2>/dev/null || true
  find "${DIST_DIR}/frontend" -type f \( -name '.eslintcache' -o -name '*.local' -o -name 'report.html' \) -delete 2>/dev/null || true
  echo "[release]   ✓ frontend/ (node_modules / dist / dist-ssr / coverage / lint-cache excluded)"
else
  echo "[release]   ⚠ frontend/ not found — 前端容器镜像缺失时将无法本地构建"
fi

# services/ ships as an empty skeleton; users create business services via skills.
mkdir -p "${DIST_DIR}/services"
if [ -f "${REPO_ROOT}/services/agents-template.md" ]; then
  cp "${REPO_ROOT}/services/agents-template.md" "${DIST_DIR}/services/agents-template.md"
fi
touch "${DIST_DIR}/services/.gitkeep"
echo "[release]   ✓ services/ (empty - for user-created business services)"

# ─── infrastructure/ (single source of truth for compose) ──────────────────
if [ ! -d "${REPO_ROOT}/infrastructure" ]; then
  echo "[release]   ✗ infrastructure/ missing at ${REPO_ROOT}/infrastructure" >&2
  exit 1
fi

mkdir -p "${DIST_DIR}/infrastructure"

# Copy every compose fragment; the main docker-compose.yml `include:`s them.
for f in \
    docker-compose.yml \
    docker-compose-infra.yml \
    docker-compose-infra.dev.yml \
    docker-compose-services.yml \
    docker-compose-observability.yml; do
  if [ -f "${REPO_ROOT}/infrastructure/${f}" ]; then
    cp "${REPO_ROOT}/infrastructure/${f}" "${DIST_DIR}/infrastructure/${f}"
  fi
done
echo "[release]   ✓ infrastructure/docker-compose*.yml"

# transform-compose.js swaps `build:` blocks for `image:` refs. Failure means
# the dist compose still points at build contexts the user does not have.
if [ ! -f "${DIST_DIR}/infrastructure/docker-compose-services.yml" ]; then
  echo "[release]   ✗ docker-compose-services.yml missing after copy" >&2
  exit 1
fi
if ! node "${ROOT}/scripts/transform-compose.js" \
    "${DIST_DIR}/infrastructure/docker-compose-services.yml" \
    "${REGISTRY}" "${VERSION}"; then
  echo "[release]   ✗ transform-compose.js failed — dist compose still has build: blocks" >&2
  exit 1
fi
echo "[release]   ✓ docker-compose-services.yml: build → image 转换完成"

# Auto-rewrite bind mounts for every service that ships a config dir.
#
# Dev-side mount is `../services/<svc>/config:<container-path>[:ro]` (relative
# to infrastructure/docker-compose-services.yml). Dist has no services/
# source tree, so rewrite to `./config/<slug>:<container-path>[:ro]`, which
# resolves under infrastructure/config/<slug>/ next to the compose file.
#
# Rewriting is delegated to release-transforms.js (unit-tested); the loop
# below only enumerates services and copies each service's config dir.
declare -A SERVICE_SLUGS=(
  [iam-service]=iam
  [notification-service]=notification
  [token-service]=token
)
services_csv=""
slug_map_csv=""
for svc_dir in "${REPO_ROOT}"/services/*/config; do
  [ -d "${svc_dir}" ] || continue
  svc=$(basename "$(dirname "${svc_dir}")")
  slug="${SERVICE_SLUGS[${svc}]:-${svc%-service}}"
  services_csv="${services_csv:+${services_csv},}${svc}"
  slug_map_csv="${slug_map_csv:+${slug_map_csv},}${svc}=${slug}"
done

if [[ -n "${services_csv}" ]]; then
  # Volume rewrite runs once, in Node, and returns a JSON report we log.
  remap_report=$(node "${ROOT}/scripts/release-transforms.js" remap-volume \
    --file "${DIST_DIR}/infrastructure/docker-compose-services.yml" \
    --services "${services_csv}" \
    --slug-map "${slug_map_csv}")
  echo "${remap_report}" | node -e '
    const data = JSON.parse(require("fs").readFileSync(0, "utf8"));
    for (const { svc, slug, matches } of data.changes) {
      const path = "../services/" + svc + "/config → ./config/" + slug;
      if (matches > 0) {
        console.log("[release]   ✓ " + svc + " volume: " + path + " (" + matches + " occurrences)");
      } else {
        console.log("[release]   ⚠ " + svc + ": no dev mount found in compose — skipping rewrite");
      }
    }
  '

  # Copy each service's dev config dir into dist infra config dir. Handled
  # in bash (not JS) because cp -a is the right primitive; we already know
  # the source/dest paths from the loop.
  for svc_dir in "${REPO_ROOT}"/services/*/config; do
    [ -d "${svc_dir}" ] || continue
    svc=$(basename "$(dirname "${svc_dir}")")
    slug="${SERVICE_SLUGS[${svc}]:-${svc%-service}}"
    mkdir -p "${DIST_DIR}/infrastructure/config/${slug}"
    cp -a "${svc_dir}/." "${DIST_DIR}/infrastructure/config/${slug}/"
    echo "[release]   ✓ infrastructure/config/${slug}/ (from services/${svc}/config/)"
  done
fi

# Non-seed config dirs (api-gateway routes, grafana provisioning, otel, etc.).
# Must run AFTER per-service seed loop so we do not clobber freshly copied
# templates; we skip any dir that already has a config.default.yaml.
if [ -d "${REPO_ROOT}/infrastructure/config" ]; then
  find "${REPO_ROOT}/infrastructure/config" -mindepth 1 -maxdepth 1 -print0 |
    while IFS= read -r -d '' item; do
      name=$(basename "${item}")
      if [ -d "${DIST_DIR}/infrastructure/config/${name}" ] &&
         [ -f "${DIST_DIR}/infrastructure/config/${name}/config.default.yaml" ]; then
        continue
      fi
      cp -a "${item}" "${DIST_DIR}/infrastructure/config/${name}"
    done
  echo "[release]   ✓ infrastructure/config/ (non-seed dirs)"
fi

for d in mysql-init nginx vendor; do
  if [ -d "${REPO_ROOT}/infrastructure/${d}" ]; then
    cp -a "${REPO_ROOT}/infrastructure/${d}" "${DIST_DIR}/infrastructure/${d}"
    echo "[release]   ✓ infrastructure/${d}/"
  fi
done

# NEVER copy infrastructure/.env — it has real dev passwords. The dist ships
# only .env.example (with __CHANGE_ME__ placeholders, from dist-template
# skeleton copy above); users run `make secrets` to fill in real values.
if [ ! -f "${DIST_DIR}/.env.example" ]; then
  echo "[release]   ⚠ .env.example missing from dist-template skeleton — user has no env template" >&2
fi

echo "[release]   ✓ infrastructure/ (Docker Compose + configs — ready for dev environment)"

# ─── Rust / node workspace stubs ──────────────────────────────────────────
if [ -f "${REPO_ROOT}/Cargo.toml" ]; then
  node "${ROOT}/scripts/release-transforms.js" filter-cargo-members \
    --in "${REPO_ROOT}/Cargo.toml" \
    --out "${DIST_DIR}/Cargo.toml"
  echo "[release]   ✓ Cargo.toml (workspace: services/ + sdk/rust/ + libs/ excluded)"
fi

mkdir -p "${DIST_DIR}/.cargo"
cat > "${DIST_DIR}/.cargo/config.toml" <<'CARGO_CONFIG'
[source.crates-io]
replace-with = "rsproxy-sparse"

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"

[source.kellnr]
registry = "sparse+http://192.168.2.12:16444/api/v1/crates/"

[net]
git-fetch-with-cli = true
CARGO_CONFIG
echo "[release]   ✓ .cargo/config.toml (kellnr registry for libs/* + sdk/rust/*)"

if [ -f "${REPO_ROOT}/pnpm-workspace.yaml" ]; then
  cp "${REPO_ROOT}/pnpm-workspace.yaml" "${DIST_DIR}/pnpm-workspace.yaml"
fi
if [ -f "${REPO_ROOT}/package.json" ]; then
  cp "${REPO_ROOT}/package.json" "${DIST_DIR}/package.json"
fi
echo "[release]   ✓ pnpm-workspace.yaml + package.json"

if [ -f "${REPO_ROOT}/AGENTS.md" ]; then
  cp "${REPO_ROOT}/AGENTS.md" "${DIST_DIR}/AGENTS.md"
  node "${ROOT}/scripts/release-transforms.js" rewrite-agents \
    --file "${DIST_DIR}/AGENTS.md"
  echo "[release]   ✓ AGENTS.md (.trae/ references replaced with .agents/)"
fi

# ─── MANIFEST.json ────────────────────────────────────────────────────────
# Records exactly what went into this build so the receiving side can audit
# provenance (which git commit, when, which images bundled). Serialization
# is delegated to release-transforms.js so the JSON shape is unit-tested.
GIT_SHA="unknown"
GIT_BRANCH="unknown"
if command -v git >/dev/null 2>&1 && git -C "${REPO_ROOT}" rev-parse --git-dir >/dev/null 2>&1; then
  GIT_SHA=$(git -C "${REPO_ROOT}" rev-parse HEAD)
  GIT_BRANCH=$(git -C "${REPO_ROOT}" rev-parse --abbrev-ref HEAD)
fi
BUILT_AT=$(date -u +'%Y-%m-%dT%H:%M:%SZ')

# Feed images list via tempfile so newlines/quoting stay clean.
IMAGES_LIST_FILE=$(mktemp)
trap 'rm -f "${IMAGES_LIST_FILE}"' EXIT
printf '%s\n' "${IMAGES[@]}" > "${IMAGES_LIST_FILE}"

node "${ROOT}/scripts/release-transforms.js" manifest \
  --out "${DIST_DIR}/MANIFEST.json" \
  --version "${VERSION}" \
  --registry "${REGISTRY}" \
  --built-at "${BUILT_AT}" \
  --git-sha "${GIT_SHA}" \
  --git-branch "${GIT_BRANCH}" \
  --export-images "${EXPORT_IMAGES}" \
  --images-file "${IMAGES_LIST_FILE}"
echo "[release]   ✓ MANIFEST.json (${VERSION} @ ${GIT_SHA:0:7})"

echo "[release] ✓ 开发骨架同步完成"
echo ""

# ─── tar ──────────────────────────────────────────────────────────────────
echo "[release] -> 打包 tar.gz..."
(cd "${ROOT}/release" && tar -czf "coder-micro-services-${VERSION}-airgap.tar.gz" \
  "coder-micro-services-${VERSION}/")

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
else
  echo "⚠ 本次未导出镜像，离线安装需先自行 docker pull 或单独执行："
  echo "    EXPORT_IMAGES=1 bash scripts/release.sh ${VERSION}"
fi
echo ""
echo "文档：分发包 docs/ 目录内有 Markdown 源可直接查看；"
echo "      完整开发文档请参见开发仓库 docs-site/ 目录。"
