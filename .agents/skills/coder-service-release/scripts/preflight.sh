#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# preflight.sh — coder-service-release 发布前预检
#
# 用法：
#   bash .agents/skills/coder-service-release/scripts/preflight.sh
#
# 检查项（任一 ERROR 立即 exit 1）：
#   1. docker / node / git 在 PATH
#   2. docker daemon 可达（docker info 5s 超时）
#   3. git 工作区干净（无 uncommitted 改动）
#   4. infrastructure/ 下无 uncommitted 改动（L1 loop 必须完成）
#   5. check-compose-commands.sh: compose command: 字段 vs Cargo [[bin]] name 全部一致
#   6. compose 中每个 service 名都能在 services/ 下找到 Cargo.toml
#   7. infrastructure/.env 存在且包含必需变量
#
# 输出末尾打印建议的发布命令（不执行）。
#
# 跨平台：Linux / macOS / Windows（Git Bash / WSL）。
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
REPO_ROOT="$(cd "${SKILL_DIR}/../.." && pwd)"

COMPOSE_FILE="${REPO_ROOT}/infrastructure/docker-compose-services.yml"
ENV_FILE="${REPO_ROOT}/infrastructure/.env"

# ── 颜色 ──────────────────────────────────────────────────────────────────
if [[ -t 1 ]]; then
  GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'
else
  GREEN=''; RED=''; YELLOW=''; CYAN=''; NC=''
fi
ok()    { printf "${GREEN}\xe2\x9c\x93${NC} %s\n" "$*"; }
fail()  { printf "${RED}\xe2\x9c\x97${NC} %s\n" "$*" >&2; exit 1; }
warn()  { printf "${YELLOW}!${NC} %s\n" "$*"; }
info()  { printf "${CYAN}i${NC} %s\n" "$*"; }

ERRORS=0

# ── 1. CLI 工具 ───────────────────────────────────────────────────────────
command -v docker >/dev/null 2>&1 || { fail "docker CLI 缺失"; ERRORS=$((ERRORS+1)); }
command -v node >/dev/null 2>&1   || { fail "node CLI 缺失"; ERRORS=$((ERRORS+1)); }
command -v git >/dev/null 2>&1    || { fail "git CLI 缺失"; ERRORS=$((ERRORS+1)); }
if [[ ${ERRORS} -gt 0 ]]; then exit 1; fi
ok "docker / node / git 均在 PATH 上"
ok "  docker: $(docker --version)"
ok "  node:   $(node --version)"
ok "  git:    $(git --version)"

# ── 2. docker daemon 可达 ────────────────────────────────────────────────
if docker info >/dev/null 2>&1; then
  ok "docker daemon 可达"
else
  warn "docker daemon 不可达（如果使用 --no-images 仍可继续；否则必须先启动 docker）"
fi

# ── 3. git 工作区干净 ────────────────────────────────────────────────────
if git -C "${REPO_ROOT}" rev-parse --git-dir >/dev/null 2>&1; then
  if git -C "${REPO_ROOT}" diff --quiet HEAD 2>/dev/null \
     && git -C "${REPO_ROOT}" diff --cached --quiet HEAD 2>/dev/null; then
    ok "工作区干净"
  else
    warn "工作区有未提交改动（可以发布，但建议先 commit）"
  fi
else
  warn "非 git 仓库，跳过工作区检查"
fi

# ── 4. infrastructure/ 干净（强制 L1 loop 完成） ────────────────────────
if git -C "${REPO_ROOT}" rev-parse --git-dir >/dev/null 2>&1; then
  if ! git -C "${REPO_ROOT}" diff --quiet -- infrastructure/ 2>/dev/null \
     || ! git -C "${REPO_ROOT}" diff --cached --quiet -- infrastructure/ 2>/dev/null; then
    fail "infrastructure/ 下有未提交改动 — 必须先经过 L1→L2 loop 完成并 commit"
  fi
  ok "infrastructure/ 工作区干净（L1 loop 已完成）"
fi

# ── 5. compose command 校验（核心：cargo bin name → 下划线） ─────────────
if [[ -f "${COMPOSE_FILE}" ]]; then
  echo
  echo "[compose] -> 检查 command: 字段 vs Cargo [[bin]] name..."
  if bash "${SCRIPT_DIR}/check-compose-commands.sh" "${COMPOSE_FILE}"; then
    ok "compose command: 字段全部与 Cargo [[bin]] name 一致"
  else
    fail "compose command: 字段存在不匹配 — 详见 check-compose-commands.sh 输出"
  fi
else
  fail "找不到 ${COMPOSE_FILE}"
fi

# ── 6. compose 中每个 service 都能在 services/ 下找到 Cargo.toml ─────────
# 用 js-yaml 精确解析（避免 awk 把 networks/volumes/secrets 顶级 key 下的 name 误判为 service）
# prue_backend 是唯一用下划线的服务目录名
COMPOSE_SERVICES="$(
  cd "${REPO_ROOT}" && node -e "
    const yaml = require('js-yaml');
    const fs = require('fs');
    const data = yaml.load(fs.readFileSync('${COMPOSE_FILE}', 'utf-8'));
    Object.keys((data && data.services) || {}).forEach(s => console.log(s));
  " 2>/dev/null
)"
if [[ -z "${COMPOSE_SERVICES}" ]]; then
  fail "无法解析 ${COMPOSE_FILE} 的 services 段（缺 js-yaml?）"
fi

MISSING=0
for svc in ${COMPOSE_SERVICES}; do
  # 直接目录匹配（kebab-case 直接,prue_backend 走下划线）
  if [[ -f "${REPO_ROOT}/services/${svc}/Cargo.toml" ]]; then
    continue
  fi
  # Fallback: outbox-publisher 类的 binary 在主服务 Cargo.toml 里
  # 例: notification-outbox-publisher → services/notification-service/Cargo.toml
  #     iam-outbox-publisher → services/iam-outbox-publisher/Cargo.toml(独立)
  MAIN_SVC="${svc%-outbox-publisher}"
  if [[ -n "${MAIN_SVC}" && "${MAIN_SVC}" != "${svc}" ]]; then
    if [[ -f "${REPO_ROOT}/services/${MAIN_SVC}/Cargo.toml" ]]; then
      continue
    fi
    if [[ -f "${REPO_ROOT}/services/${MAIN_SVC}-service/Cargo.toml" ]]; then
      continue
    fi
  fi
  # prue_backend 用下划线目录
  if [[ -f "${REPO_ROOT}/services/prue_backend/Cargo.toml" ]]; then
    continue
  fi
  warn "compose 引用 service '${svc}'，但 services/${svc}/ 下找不到 Cargo.toml"
  MISSING=$((MISSING+1))
done
if [[ ${MISSING} -eq 0 ]]; then
  SVC_COUNT=$(echo "${COMPOSE_SERVICES}" | wc -l | tr -d ' ')
  ok "compose 中所有 ${SVC_COUNT} 个 service 都能在 services/ 下找到（直接或 fallback）"
else
  fail "${MISSING} 个 compose service 找不到对应 Cargo.toml"
fi

# ── 7. infrastructure/.env 完整性 ───────────────────────────────────────
echo
echo "[env] -> 检查 ${ENV_FILE}..."
if [[ ! -f "${ENV_FILE}" ]]; then
  warn "找不到 ${ENV_FILE}（compose 模板会展开空值，可能导致容器启动失败）"
  warn "  参考 .env.example 创建（如果存在）"
else
  REQUIRED_VARS=(
    "MYSQL_ROOT_PASSWORD"
    "MYSQL_PASSWORD"
    "JWKS_URL"
    "DATABASE_URL"
  )
  MISSING_VARS=0
  for v in "${REQUIRED_VARS[@]}"; do
    if ! grep -qE "^${v}=" "${ENV_FILE}"; then
      warn ".env 缺必需变量: ${v}"
      MISSING_VARS=$((MISSING_VARS+1))
    fi
  done
  if [[ ${MISSING_VARS} -eq 0 ]]; then
    ok ".env 包含全部必需变量"
  else
    warn ".env 缺 ${MISSING_VARS} 个变量（soft warning；请人工确认 compose 是否能正常启动）"
  fi
fi

# ── 报告 ─────────────────────────────────────────────────────────────────
echo
echo "============================================"
echo " Release Plan (review, then run)"
echo "============================================"
echo "  Registry: zhcoder-docker-registry.com:8000"
echo "  VERSION:  <next-version, e.g. v1.2.0>"
echo
echo "  1. 构建并推送镜像（如需要）:"
echo "     pnpm run docker:build -- --load-and-push -v <VERSION-NUMBER>"
echo
echo "  2. 打 airgap 包（含镜像）:"
echo "     pnpm run release:build"
echo
echo "  3. 打 deploy-only 包（不含 dev 工具）:"
echo "     pnpm run release:deploy"
echo
echo "  4. 预览骨架（不导出镜像，10s 跑完）:"
echo "     EXPORT_IMAGES=0 node release/dist-template/scripts/release.js <VERSION> --no-images"
echo "============================================"
echo
ok "全部预检通过 — 可以执行 release"
