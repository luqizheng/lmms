#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# sdk/scripts/publish-rust-sdks.sh - 发布 Rust SDK crates 到私有 registry
#
# 依赖顺序：先发布 digua-http-client（基础库），再发布 6 个 service SDK。
# 各 service SDK 通过 path 依赖 digua-http-client，发布时临时替换为 version
# 依赖，发布完自动恢复。
#
# 前置条件：
#   1. `cargo login --registry <registry>` 已完成（setup-wizard 会写
#      ~/.cargo/credentials.toml 与 .cargo/config.toml）
#   2. 网络可访问目标 registry（默认 kellnr，--registry 指定其他）
#   3. 工作区能正常编译（`cargo build -p digua-http-client` 通过）
#
# 用法：
#   bash sdk/scripts/publish-rust-sdks.sh <VERSION> [--dry-run] [--registry <name>]
#
# 必填位置参数：
#   VERSION            统一版本号（例如 0.2.0）；所有 7 个 crate 改到该版本
#
# 可选 flag：
#   --dry-run          只验证，不实际发布
#   --registry NAME    发布到指定 registry（默认 kellnr）
#   --help, -h         打印帮助
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

# ── 默认值 ──────────────────────────────────────────────────────────────────
CRATE_VERSION=""
DRY_RUN=0
REGISTRY="kellnr"
SED_INPLACE=(-i)

# ── 路径 ────────────────────────────────────────────────────────────────────
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RUST_SDK_DIR="${ROOT}/rust"

# 发布顺序：基础库在前，service SDK 在后
BASE_CRATES=(
  "digua-http-client"
)
SERVICE_CRATES=(
  "audit-service-sdk"
  "config-service-sdk"
  "file-service-sdk"
  "iam-service-sdk"
  "notification-service-sdk"
  "token-service-sdk"
)

# ── 参数解析 ────────────────────────────────────────────────────────────────
SHOW_HELP=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)  DRY_RUN=1; shift ;;
    --registry) REGISTRY="${2:-}"; shift 2 ;;
    --help|-h)  SHOW_HELP=1; shift ;;
    -*)         echo "[publish-rust] 未知参数: $1" >&2; SHOW_HELP=1; shift ;;
    *)
      if [[ -z "${CRATE_VERSION}" ]]; then
        CRATE_VERSION="$1"; shift
      else
        echo "[publish-rust] 未知参数: $1" >&2; SHOW_HELP=1; shift
      fi
      ;;
  esac
done

if [[ "${SHOW_HELP}" == "1" ]]; then
  sed -n '2,28p' "$0" | sed 's/^# \{0,1\}//'
  exit 0
fi

if [[ -z "${CRATE_VERSION}" ]]; then
  echo "[publish-rust] ERROR: VERSION 是必填位置参数（例如 0.2.0）" >&2
  echo "用法: bash sdk/scripts/publish-rust-sdks.sh <VERSION> [--dry-run] [--registry <name>]" >&2
  exit 2
fi

echo "============================================"
echo " Rust SDK Publish"
echo " Version:  ${CRATE_VERSION}"
echo " Registry: ${REGISTRY}"
echo " Dry-run:  ${DRY_RUN}"
echo "============================================"
echo ""

# ── 前置检查 ────────────────────────────────────────────────────────────────
if ! command -v cargo &>/dev/null; then
  echo "[publish-rust] ERROR: 'cargo' not found on PATH" >&2
  exit 1
fi

# 检查每个 crate 的必要 package 字段
REQUIRED_FIELDS=("description" "license")
check_required_fields() {
  local crate_dir="$1"
  local crate_name
  crate_name="$(basename "${crate_dir}")"
  for field in "${REQUIRED_FIELDS[@]}"; do
    if ! grep -q "^${field} " "${crate_dir}/Cargo.toml"; then
      echo "[publish-rust] ERROR: ${crate_name} missing '${field}' in Cargo.toml — required by crates.io" >&2
      exit 1
    fi
  done
}

echo "[publish-rust] -> Checking required fields..."
for crate in "${BASE_CRATES[@]}" "${SERVICE_CRATES[@]}"; do
  check_required_fields "${RUST_SDK_DIR}/${crate}"
done
echo "[publish-rust]   ✓ All crates have required fields"
echo ""

# ── 统一版本号 ──────────────────────────────────────────────────────────────
echo "[publish-rust] -> Ensuring all crates have version ${CRATE_VERSION}..."
for crate in "${BASE_CRATES[@]}" "${SERVICE_CRATES[@]}"; do
  toml="${RUST_SDK_DIR}/${crate}/Cargo.toml"
  current="$(grep "^version " "${toml}" | sed 's/version = "\(.*\)"/\1/')"
  if [[ "${current}" != "${CRATE_VERSION}" ]]; then
    if [[ "${DRY_RUN}" == "1" ]]; then
      echo "[publish-rust]   ~ ${crate}: ${current} -> ${CRATE_VERSION}  (dry-run; not modified)"
    else
      echo "[publish-rust]   ~ ${crate}: ${current} -> ${CRATE_VERSION}"
      sed "${SED_INPLACE[@]}" "s/^version = \"${current}\"/version = \"${CRATE_VERSION}\"/" "${toml}"
    fi
  else
    echo "[publish-rust]   ✓ ${crate}: ${CRATE_VERSION}"
  fi
done
echo ""

# ── 注册 registry 参数 ─────────────────────────────────────────────────────
REGISTRY_ARG=()
REGISTRY_TOML=""
if [[ -n "${REGISTRY}" ]]; then
  REGISTRY_ARG=(--registry "${REGISTRY}")
  REGISTRY_TOML="${REGISTRY}"
fi

# ── 发布函数 ────────────────────────────────────────────────────────────────
publish_crate() {
  local crate_name="$1"
  shift

  echo "[publish-rust]   Publishing ${crate_name}..."
  if [[ "${DRY_RUN}" == "1" ]]; then
    echo "[publish-rust]     (dry-run) cargo publish -p ${crate_name} ${REGISTRY_ARG[*]:+--registry ${REGISTRY}} --no-verify --allow-dirty"
    if [[ -n "${REGISTRY}" ]]; then
      cargo publish -p "${crate_name}" --registry "${REGISTRY}" --no-verify --allow-dirty --dry-run 2>&1 | sed 's/^/      /' || true
    else
      cargo publish -p "${crate_name}" --no-verify --allow-dirty --dry-run 2>&1 | sed 's/^/      /' || true
    fi
  else
    if [[ -n "${REGISTRY}" ]]; then
      cargo publish -p "${crate_name}" --registry "${REGISTRY}" --no-verify --allow-dirty 2>&1 | sed 's/^/      /'
    else
      cargo publish -p "${crate_name}" --no-verify --allow-dirty 2>&1 | sed 's/^/      /'
    fi
    local rc=${PIPESTATUS[0]}
    if [[ ${rc} -ne 0 ]]; then
      echo "[publish-rust]   ✗ ${crate_name} publish failed (exit ${rc})" >&2
      return 1
    fi
  fi
  echo "[publish-rust]   ✓ ${crate_name} published"
}

# ── 发布 digua-http-client（基础库，无 path 依赖问题）───────────────────
echo "[publish-rust] -> Step 1/2: Publishing base crate ${BASE_CRATES[0]}..."
(
  cd "${RUST_SDK_DIR}/${BASE_CRATES[0]}"
  publish_crate "${BASE_CRATES[0]}"
)
echo ""

# ── 发布 service SDK（需临时替换 path 依赖为 version 依赖）───────────────
echo "[publish-rust] -> Step 2/2: Publishing service SDK crates..."

BACKUP_FILES=()

if [[ "${DRY_RUN}" == "1" ]]; then
  echo "[publish-rust]   (dry-run) would replace path deps with version+registry, then publish each crate"
else
  for crate in "${SERVICE_CRATES[@]}"; do
    toml="${RUST_SDK_DIR}/${crate}/Cargo.toml"
    # 必须带 registry = "${REGISTRY}"，否则 cargo 从 crates-io 找 digua-http-client 找不到
    if grep -q "path = \"../digua-http-client\"" "${toml}"; then
      echo "[publish-rust]   ~ ${crate}: replacing path dep with version ${CRATE_VERSION} + registry ${REGISTRY}"
      sed "${SED_INPLACE[@]}.bak" \
        's|digua_http_client = { path = "../digua-http-client" }|digua_http_client = { version = "'"${CRATE_VERSION}"'", registry = "'"${REGISTRY}"'" }|' \
        "${toml}"
      BACKUP_FILES+=("${toml}")
    fi
  done
fi

# 逐个发布
PUBLISH_FAILED=0
for crate in "${SERVICE_CRATES[@]}"; do
  (
    cd "${RUST_SDK_DIR}/${crate}"
    publish_crate "${crate}"
  ) || {
    echo "[publish-rust]   ✗ ${crate} failed" >&2
    PUBLISH_FAILED=1
  }
done

# ── 恢复 path 依赖 ──────────────────────────────────────────────────────────
echo ""
echo "[publish-rust] -> Restoring path dependencies..."
for toml in "${BACKUP_FILES[@]}"; do
  if [[ -f "${toml}.bak" ]]; then
    mv "${toml}.bak" "${toml}"
    echo "[publish-rust]   ✓ restored ${toml}"
  fi
done

# ── 结果 ────────────────────────────────────────────────────────────────────
echo ""
if [[ "${DRY_RUN}" == "1" ]]; then
  echo "============================================"
  echo " Dry-run complete. No crates were published."
  echo " Run without --dry-run to publish."
  echo "============================================"
elif [[ "${PUBLISH_FAILED}" == "1" ]]; then
  echo "============================================"
  echo " WARNING: Some crates failed to publish."
  echo " Re-run to retry failed ones (idempotent)."
  echo "============================================"
  exit 1
else
  echo "============================================"
  echo " All Rust SDK crates published successfully!"
  echo " Version: ${CRATE_VERSION}"
  [[ -n "${REGISTRY}" ]] && echo " Registry: ${REGISTRY}" || echo " Registry: crates.io"
  echo "============================================"
  echo ""
  echo "Next step: consumers add via Cargo.toml:"
  echo "  [dependencies]"
  echo "  iam-service-sdk = \"${CRATE_VERSION}\""
  echo "  # or: cargo add iam-service-sdk"
fi
