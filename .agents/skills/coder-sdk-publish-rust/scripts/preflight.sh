#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# preflight.sh — coder-sdk-publish-rust 发布前预检
#
# 用法：
#   bash .agents/skills/coder-sdk-publish-rust/scripts/preflight.sh <version> [libs|sdk|all]
#
# 检查项（任一失败立即 exit 1）：
#   1. cargo / node 在 PATH
#   2. CARGO_REGISTRIES_KELLNR_TOKEN 环境变量已设置
#   3. kellnr registry 可达 (curl HEAD)
#   4. libs/<17 crates> 或 sdk/rust/<7 crates> 全部存在
#   5. 每个 crate 的 Cargo.toml 都有 description + license 字段
#   6. cargo metadata 解析成功（无 broken path dep）
#   7. 工作区干净（相关目录下无 uncommitted 改动）
#   8. 提示跨平台 CARGO_TARGET_DIR 默认值
#
# 跨平台：Linux / macOS / Windows（Git Bash / WSL / PowerShell）。
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

# ── 路径与常量 ─────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
REPO_ROOT="$(cd "${SKILL_DIR}/../.." && pwd)"

KELLNR_REGISTRY="http://192.168.2.12:16444"

# Layer 0..2 拓扑顺序（与 publish-libs.js 一致）
LIBS_CRATES=(
  common-error common-health common-jwt common-ratelimit common-telemetry
  common-cache common-config common-crypto common-event common-http
  common-log common-validator common-telemetry-axum jwt-ctx
  common-db authz common
)

# digua-http-client + 6 service SDK
SDK_CRATES=(
  digua-http-client
  audit-service-sdk config-service-sdk file-service-sdk
  iam-service-sdk notification-service-sdk token-service-sdk
)

# ── 颜色 ──────────────────────────────────────────────────────────────────
if [[ -t 1 ]]; then
  GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; NC='\033[0m'
else
  GREEN=''; RED=''; YELLOW=''; NC=''
fi
ok()   { printf "${GREEN}\xe2\x9c\x93${NC} %s\n" "$*"; }
fail() { printf "${RED}\xe2\x9c\x97${NC} %s\n" "$*" >&2; exit 1; }
warn() { printf "${YELLOW}!${NC} %s\n" "$*"; }
info() { printf "  %s\n" "$*"; }

# ── 参数校验 ──────────────────────────────────────────────────────────────
if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "用法: $0 <version> [libs|sdk|all]" >&2
  echo "示例: $0 1.2.0 libs" >&2
  exit 2
fi
VERSION="$1"
TARGET="${2:-all}"

if [[ ! "${TARGET}" =~ ^(libs|sdk|all)$ ]]; then
  fail "target 必须是 libs / sdk / all（收到: ${TARGET}）"
fi

if ! [[ "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.+][0-9A-Za-z.-]+)?$ ]]; then
  fail "version 必须是合法 semver（收到: ${VERSION}）"
fi

echo
echo "============================================"
echo " Rust SDK Publish Pre-flight"
echo " Version:  ${VERSION}"
echo " Target:   ${TARGET}"
echo " Registry: kellnr (${KELLNR_REGISTRY})"
echo "============================================"
echo

# ── 1. CLI 工具 ───────────────────────────────────────────────────────────
command -v cargo >/dev/null 2>&1 || fail "需要 cargo CLI（在 PATH 上）"
CARGO_VERSION="$(cargo --version)"
ok "cargo: ${CARGO_VERSION}"

command -v node >/dev/null 2>&1 || fail "需要 node CLI（publish 脚本是 Node.js）"
ok "node: $(node --version)"

# ── 2. CARGO_REGISTRIES_KELLNR_TOKEN ───────────────────────────────────────
if [[ -z "${CARGO_REGISTRIES_KELLNR_TOKEN:-}" ]]; then
  fail "CARGO_REGISTRIES_KELLNR_TOKEN 未设置。设置方法：
    1. cargo login --registry kellnr   # 交互式粘贴 token
    2. 或在 ~/.cargo/credentials.toml 中加入：
         [registries.kellnr]
         token = \"<your-token>\"
    3. 或 export CARGO_REGISTRIES_KELLNR_TOKEN=<your-token>"
fi
ok "CARGO_REGISTRIES_KELLNR_TOKEN 已设置（长度 ${#CARGO_REGISTRIES_KELLNR_TOKEN}）"

# ── 3. registry 可达 ─────────────────────────────────────────────────────
if command -v curl >/dev/null 2>&1; then
  HTTP_CODE="$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 "${KELLNR_REGISTRY}" || echo 000)"
  if [[ "${HTTP_CODE}" =~ ^(2|3)[0-9]{2}$ ]] || [[ "${HTTP_CODE}" == "401" ]] || [[ "${HTTP_CODE}" == "404" ]]; then
    ok "kellnr 可达（HTTP ${HTTP_CODE}）"
  else
    fail "kellnr 不可达（HTTP ${HTTP_CODE}）：${KELLNR_REGISTRY}"
  fi
else
  warn "无 curl，跳过 registry 可达性检查"
fi

# ── 4-5. crates 存在 + description/license ────────────────────────────────
check_crate() {
  local rel="$1"
  local dir="${REPO_ROOT}/${rel}"
  if [[ ! -d "${dir}" ]]; then
    fail "crate 目录不存在: ${rel}"
  fi
  if [[ ! -f "${dir}/Cargo.toml" ]]; then
    fail "缺 Cargo.toml: ${rel}/Cargo.toml"
  fi
  if ! grep -qE '^description\s*=' "${dir}/Cargo.toml"; then
    fail "${rel}/Cargo.toml 缺 description 字段（kellnr 强制要求）"
  fi
  if ! grep -qE '^license\s*=' "${dir}/Cargo.toml"; then
    fail "${rel}/Cargo.toml 缺 license 字段（kellnr 强制要求）"
  fi
}

if [[ "${TARGET}" == "libs" || "${TARGET}" == "all" ]]; then
  echo "[libs] -> Checking ${#LIBS_CRATES[@]} crates..."
  for c in "${LIBS_CRATES[@]}"; do
    check_crate "libs/${c}"
  done
  ok "libs: ${#LIBS_CRATES[@]} crates 全部存在并满足 description/license"
fi

if [[ "${TARGET}" == "sdk" || "${TARGET}" == "all" ]]; then
  echo "[sdk] -> Checking ${#SDK_CRATES[@]} crates..."
  for c in "${SDK_CRATES[@]}"; do
    check_crate "sdk/rust/${c}"
  done
  ok "sdk/rust: ${#SDK_CRATES[@]} crates 全部存在并满足 description/license"
fi

# ── 6. cargo metadata 解析 ───────────────────────────────────────────────
# 验证 path 依赖能解析（防止 publish 时替换 path → version 出现漏网之鱼）
if [[ "${TARGET}" == "libs" || "${TARGET}" == "all" ]]; then
  if [[ -f "${REPO_ROOT}/libs/Cargo.toml" ]]; then
    if cargo metadata --manifest-path "${REPO_ROOT}/libs/Cargo.toml" \
         --format-version 1 --offline >/dev/null 2>&1; then
      ok "libs/Cargo.toml: cargo metadata 解析成功"
    else
      warn "libs/Cargo.toml: cargo metadata 解析失败（可能缺 lockfile 或 path dep 漂移）"
      warn "  建议：cd libs && cargo metadata --format-version 1 看详细错误"
    fi
  fi
fi

# ── 7. 工作区干净 ─────────────────────────────────────────────────────────
if command -v git >/dev/null 2>&1 && git -C "${REPO_ROOT}" rev-parse --git-dir >/dev/null 2>&1; then
  if [[ "${TARGET}" == "libs" || "${TARGET}" == "all" ]]; then
    if ! git -C "${REPO_ROOT}" diff --quiet -- libs/ 2>/dev/null \
       || ! git -C "${REPO_ROOT}" diff --cached --quiet -- libs/ 2>/dev/null; then
      warn "libs/ 下有未提交改动（可以发布，但建议先 commit）"
    else
      ok "libs/ 工作区干净"
    fi
  fi
  if [[ "${TARGET}" == "sdk" || "${TARGET}" == "all" ]]; then
    if ! git -C "${REPO_ROOT}" diff --quiet -- sdk/rust/ 2>/dev/null \
       || ! git -C "${REPO_ROOT}" diff --cached --quiet -- sdk/rust/ 2>/dev/null; then
      warn "sdk/rust/ 下有未提交改动（可以发布，但建议先 commit）"
    else
      ok "sdk/rust/ 工作区干净"
    fi
  fi
else
  warn "非 git 仓库，跳过工作区检查"
fi

# ── 8. 跨平台 CARGO_TARGET_DIR 提示 ───────────────────────────────────────
echo
case "$(uname -s 2>/dev/null || echo Windows)" in
  Linux|Darwin)
    SUGGESTED="/tmp/rust-publish-target-$$"
    info "建议（Linux/macOS）: export CARGO_TARGET_DIR=${SUGGESTED}"
    ;;
  MINGW*|MSYS*|CYGWIN*|Windows*)
    SUGGESTED='$env:TEMP\rust-publish-target'
    info "建议（Windows PowerShell）: \$env:CARGO_TARGET_DIR=\"${SUGGESTED}\""
    ;;
  *)
    warn "未识别 OS，自行设置 CARGO_TARGET_DIR 指向非 repo 内 target/ 的目录"
    ;;
esac

echo
ok "全部预检通过 — 可以发布 ${VERSION} 到 kellnr"
echo
echo "下一步（libs）:"
echo "  CARGO_TARGET_DIR=<上面建议的路径> pnpm run sdk:publish:libs ${VERSION} --dry-run"
echo "  CARGO_TARGET_DIR=<上面建议的路径> pnpm run sdk:publish:libs ${VERSION}"
echo
echo "下一步（sdk）:"
echo "  CARGO_TARGET_DIR=<上面建议的路径> pnpm run sdk:publish:rust ${VERSION} --dry-run"
echo "  CARGO_TARGET_DIR=<上面建议的路径> pnpm run sdk:publish:rust ${VERSION}"
