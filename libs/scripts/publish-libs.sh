#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# libs/scripts/publish-libs.sh - 发布 libs/ 共享库 crates 到私有 registry
#
# 按拓扑顺序发布 17 个 crate（common-error / common-db / common-jwt / authz 等），
# 临时把内部 path 依赖替换为 version 依赖，发布完自动恢复。
#
# 依赖顺序（拓扑）：
#   Layer 0: common-error, common-health, common-jwt, common-ratelimit, common-telemetry
#   Layer 1: common-cache, common-config, common-crypto, common-event,
#            common-http, common-log, common-validator, common-telemetry-axum,
#            jwt-ctx
#   Layer 2: common-db (->error, jwt-ctx), authz (->jwt-ctx, common-jwt),
#            common (->error, db, crypto)
#
# 前置条件：
#   1. `cargo login --registry <registry>` 已完成，或 ~/.cargo/credentials 已配置 token
#      （setup-wizard 会写 ~/.cargo/credentials.toml 与 .cargo/config.toml）
#   2. 网络可访问目标 registry
#   3. 工作区能正常编译（`cargo build -p common-error` 通过）
#
# 用法：
#   bash libs/scripts/publish-libs.sh <VERSION> [--dry-run] [--registry <name>]
#
# 必填位置参数：
#   VERSION            统一版本号（例如 0.2.0）；所有 17 个 crate 改到该版本
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
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LIBS_DIR="${ROOT}/libs"

# 发布顺序：拓扑排序（被依赖的在前）
# Layer 0: 无 lib 依赖
# Layer 1: 仅依赖 Layer 0
# Layer 2: 依赖 Layer 0+1
PUBLISH_ORDER=(
  # Layer 0
  "common-error"
  "common-health"
  "common-jwt"
  "common-ratelimit"
  "common-telemetry"
  # Layer 1
  "common-cache"
  "common-config"
  "common-crypto"
  "common-event"
  "common-http"
  "common-log"
  "common-validator"
  "common-telemetry-axum"
  "jwt-ctx"
  # Layer 2
  "common-db"
  "authz"
  "common"
)

# ── 参数解析 ────────────────────────────────────────────────────────────────
SHOW_HELP=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)  DRY_RUN=1; shift ;;
    --registry) REGISTRY="${2:-}"; shift 2 ;;
    --help|-h)  SHOW_HELP=1; shift ;;
    -*)         echo "[publish-libs] 未知参数: $1" >&2; SHOW_HELP=1; shift ;;
    *)
      if [[ -z "${CRATE_VERSION}" ]]; then
        CRATE_VERSION="$1"; shift
      else
        echo "[publish-libs] 未知参数: $1" >&2; SHOW_HELP=1; shift
      fi
      ;;
  esac
done

if [[ "${SHOW_HELP}" == "1" ]]; then
  sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'
  exit 0
fi

if [[ -z "${CRATE_VERSION}" ]]; then
  echo "[publish-libs] ERROR: VERSION 是必填位置参数（例如 0.2.0）" >&2
  echo "用法: bash libs/scripts/publish-libs.sh <VERSION> [--dry-run] [--registry <name>]" >&2
  exit 2
fi

echo "============================================"
echo " Libs Publish"
echo " Version:  ${CRATE_VERSION}"
echo " Registry: ${REGISTRY}"
echo " Dry-run:  ${DRY_RUN}"
echo "============================================"
echo ""

# ── 前置检查 ────────────────────────────────────────────────────────────────
if ! command -v cargo &>/dev/null; then
  echo "[publish-libs] ERROR: 'cargo' not found on PATH" >&2
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
      echo "[publish-libs] ERROR: ${crate_name} missing '${field}' in Cargo.toml - required by registry" >&2
      exit 1
    fi
  done
}

echo "[publish-libs] -> Checking required fields..."
for crate in "${PUBLISH_ORDER[@]}"; do
  check_required_fields "${LIBS_DIR}/${crate}"
done
echo "[publish-libs]   ✓ All crates have required fields"
echo ""

# ── 统一版本号 ──────────────────────────────────────────────────────────────
echo "[publish-libs] -> Ensuring all crates have version ${CRATE_VERSION}..."
for crate in "${PUBLISH_ORDER[@]}"; do
  toml="${LIBS_DIR}/${crate}/Cargo.toml"
  current="$(grep "^version " "${toml}" | sed 's/version = "\(.*\)"/\1/')"
  if [[ "${current}" != "${CRATE_VERSION}" ]]; then
    if [[ "${DRY_RUN}" == "1" ]]; then
      echo "[publish-libs]   ~ ${crate}: ${current} -> ${CRATE_VERSION}  (dry-run; not modified)"
    else
      echo "[publish-libs]   ~ ${crate}: ${current} -> ${CRATE_VERSION}"
      sed "${SED_INPLACE[@]}" "s/^version = \"${current}\"/version = \"${CRATE_VERSION}\"/" "${toml}"
    fi
  else
    echo "[publish-libs]   ✓ ${crate}: ${CRATE_VERSION}"
  fi
done
echo ""

# ── 替换内部 path 依赖为 version + registry 依赖 ─────────────────────────
# libs/ 内部的 path 依赖形如：
#   common-error = { path = "../common-error" }
#   common-db = { path = "../common-db", features = ["mysql"] }
# 发布时需要替换为：
#   common-error = { version = "0.1.0", registry = "kellnr" }
#   common-db = { version = "0.1.0", registry = "kellnr", features = ["mysql"] }
#
# 必须带 registry = "${REGISTRY}"，否则 cargo 从 crates-io 找，找不到。
#
# 策略：逐个 crate 替换+发布+恢复（不提前替换所有）。
# 原因：如果提前替换所有 Cargo.toml，workspace 解析时 `common` 依赖 kellnr 上
# 还不存在的 `common-crypto`，导致所有 crate 发布失败。
# 逐个替换时，其他 crate 保持 path 依赖（能解析），只有当前 crate 用 version+registry。

# 内部 crate 名集合（用于识别哪些 path 依赖是 libs/ 内部的）
INTERNAL_CRATES=("${PUBLISH_ORDER[@]}")

echo "[publish-libs] -> Using per-crate replace+publish+restore strategy"
echo ""

# ── 发布函数 ────────────────────────────────────────────────────────────────
publish_crate() {
  local crate_name="$1"

  echo "[publish-libs]   Publishing ${crate_name}..."
  if [[ "${DRY_RUN}" == "1" ]]; then
    echo "[publish-libs]     (dry-run) cargo publish -p ${crate_name} --registry ${REGISTRY} --no-verify --allow-dirty --dry-run"
    cargo publish -p "${crate_name}" --registry "${REGISTRY}" --no-verify --allow-dirty --dry-run 2>&1 | sed 's/^/      /' || true
  else
    cargo publish -p "${crate_name}" --registry "${REGISTRY}" --no-verify --allow-dirty 2>&1 | sed 's/^/      /'
    local rc=${PIPESTATUS[0]}
    if [[ ${rc} -ne 0 ]]; then
      echo "[publish-libs]   ✗ ${crate_name} publish failed (exit ${rc})" >&2
      return 1
    fi
  fi
  echo "[publish-libs]   ✓ ${crate_name} published"
}

# ── 按拓扑顺序逐个发布（替换+发布+恢复）────────────────────────────────
echo "[publish-libs] -> Publishing crates in topological order (per-crate replace+publish+restore)..."
echo ""

PUBLISH_FAILED=0
for crate in "${PUBLISH_ORDER[@]}"; do
  toml="${LIBS_DIR}/${crate}/Cargo.toml"
  modified=0

  # 替换当前 crate 的 path 依赖为 version + registry
  for internal in "${INTERNAL_CRATES[@]}"; do
    if grep -q "${internal} = { path = \"\.\./${internal}\"" "${toml}"; then
      if [[ ${modified} -eq 0 ]]; then
        cp "${toml}" "${toml}.bak"
        modified=1
      fi
      sed "${SED_INPLACE[@]}" \
        "s|${internal} = { path = \"\.\./${internal}\" }|${internal} = { version = \"${CRATE_VERSION}\", registry = \"${REGISTRY}\" }|g" \
        "${toml}"
      sed "${SED_INPLACE[@]}" \
        "s|${internal} = { path = \"\.\./${internal}\", |${internal} = { version = \"${CRATE_VERSION}\", registry = \"${REGISTRY}\", |g" \
        "${toml}"
      echo "[publish-libs]   ~ ${crate}: ${internal} path -> version + registry"
    fi
  done

  # 发布当前 crate
  (
    cd "${LIBS_DIR}/${crate}"
    publish_crate "${crate}"
  ) || {
    echo "[publish-libs]   ✗ ${crate} failed" >&2
    PUBLISH_FAILED=1
  }

  # 恢复当前 crate 的 Cargo.toml
  if [[ ${modified} -eq 1 ]]; then
    mv "${toml}.bak" "${toml}"
    echo "[publish-libs]   ~ restored ${crate}/Cargo.toml"
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
  echo " All libs crates published successfully!"
  echo " Version: ${CRATE_VERSION}"
  echo " Registry: ${REGISTRY}"
  echo "============================================"
  echo ""
  echo "Next step: consumers add via Cargo.toml:"
  echo "  [dependencies]"
  echo "  common-error = { version = \"${CRATE_VERSION}\", registry = \"${REGISTRY}\" }"
  echo "  # or: CARGO_REGISTRIES_KELLNR_TOKEN=xxx cargo add common-error --registry ${REGISTRY}"
fi
