#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# swap-staging-active.sh — 原子交换 staging-active 软链指向新 SHA 版本
#
# 由 .gitlab-ci.yml 的 deploy:staging / e2e:staging 调用（在 staging server 上）
# 核心不变量：staging-active 是唯一可变状态，原子交换；其他 staging-<sha>/ 不可变。
#
# 用法：
#   bash infrastructure/scripts/swap-staging-active.sh <new-sha> [--dry-run|--force]
#
# 行为：
#   1. 校验目标 staging-<new-sha>/ 目录存在
#   2. 复用上版的 secrets/ + .env（避免每次 reset 密钥；同 dir 跳过 self-copy）
#   3. 把当前 active（旧版）退到 staging-prev（rollback 锚点；同 dir 跳过以免覆盖 prev）
#   4. ln -sfn 原子替换 staging-active 软链到新 SHA（atomic on Linux）
#   5. 重起容器（~30s downtime，staging 接受）
#
# Idempotent 处理（#25062 fix）：active 已指向 NEW_DIR 时用 status.js 探针；
# 都 alive 则 exit 0；不健康或 --force fall through 触发 stop+start。
# 此时 PREV==NEW_DIR，step 2/3 跳（避免 secrets 自复制 + 覆盖真 staging-prev）。
#
# 回滚：
#   bash infrastructure/scripts/swap-staging-active.sh <old-sha>
#   （新版的 staging-<old-sha>/ 还在，下一次 swap 会自动复位）
# ═══════════════════════════════════════════════════════════════════════════════
set -euo pipefail

NEW_SHA="${1:-}"
DRY_RUN=0
FORCE=0
SAME_DIR=0
for arg in "${@:2}"; do
  case "${arg}" in
    --dry-run) DRY_RUN=1 ;;
    --force)   FORCE=1 ;;
    *) echo "[swap] 未知参数: ${arg}" >&2; exit 2 ;;
  esac
done

[ -n "${NEW_SHA}" ] || { echo "[swap] usage: swap-staging-active.sh <sha> [--dry-run|--force]" >&2; exit 1; }

BASE="${STAGING_BASE_DIR:-/home/kudi/coder-micro-services}"
ACTIVE_LINK="${BASE}/staging-active"
PREV_LINK="${BASE}/staging-prev"
NEW_DIR="${BASE}/staging-${NEW_SHA}"

[ -d "${NEW_DIR}" ] || { echo "[swap] ✗ ${NEW_DIR} 不存在（先 rsync deploy 骨架到 staging server）" >&2; exit 1; }

# 1. 计算当前 active（symlink target）
PREV=""
if [ -L "${ACTIVE_LINK}" ]; then
  PREV="$(readlink -f "${ACTIVE_LINK}" || true)"
fi

if [ -z "${PREV}" ]; then
  echo "[swap] ⚠ staging-active 不存在或断链；首次部署？直接建链"
elif [ "${PREV}" = "${NEW_DIR}" ]; then
  echo "[swap] ⚠ active 已指向 ${NEW_SHA}，验证容器是否仍 alive"
  cd "${NEW_DIR}"
  SAME_DIR=1
  if [ "${FORCE}" = "1" ]; then
    echo "[swap] --force：跳过 alive 探针，强制重启"
  elif ps_out="$(node bin/status.js 2>/dev/null || true)" \
       && echo "${ps_out}" | grep -qiE '\b(Up\b|healthy)'; then
    echo "[swap] ✓ 容器仍 alive（status.js 探针），无操作"
    exit 0
  else
    echo "[swap] ⚠ active 指向 OK 但容器不健康，触发 stop+start"
  fi
fi

if [ "${DRY_RUN}" = "1" ]; then
  echo "[swap] --dry-run: 将做"
  echo "[swap]   prev=${PREV:-<none>}"
  echo "[swap]   new=${NEW_DIR}"
  echo "[swap]   same-dir=${SAME_DIR}（1=active 已指 NEW_DIR，跳 step 2/3）"
  if [ -n "${PREV}" ] && [ -d "${PREV}" ]; then
    echo "[swap]   复用 prev secrets/ + .env 到 new"
  elif [ ! -f "${NEW_DIR}/.env" ] && [ -f "${NEW_DIR}/.env.example" ]; then
    echo "[swap]   从 ${NEW_DIR}/.env.example 初始化 .env（首次部署，无 prev 可复用）"
  else
    echo "[swap]   new 已存在 .env，跳过初始化"
  fi
  echo "[swap]   ln -sfn ${PREV:-<none>} ${PREV_LINK}"
  echo "[swap]   ln -sfn ${NEW_DIR} ${ACTIVE_LINK}"
  echo "[swap]   cd ${NEW_DIR} && node bin/stop.js && node bin/start.js --deploy"
  exit 0
fi

# 2. 复用 secrets + .env（避免 reset JWT key）
if [ "${SAME_DIR}" = "0" ] && [ -n "${PREV}" ] && [ -d "${PREV}" ]; then
  if [ -d "${PREV}/secrets" ]; then
    cp -a "${PREV}/secrets/." "${NEW_DIR}/secrets/" 2>/dev/null || true
    echo "[swap] ✓ 复用 secrets/ from prev"
  fi
  if [ -f "${PREV}/.env" ]; then
    cp -a "${PREV}/.env" "${NEW_DIR}/.env" 2>/dev/null || true
    echo "[swap] ✓ 复用 .env from prev"
  fi
fi

# 2.5 .env 兜底：NEW_DIR 缺 .env 时无条件从 .env.example 复制（job 25200 踩到 PREV==NEW_DIR
# fallthrough + SAME_DIR=1 跳过 step 2 的边界；首次部署 PREV="" 也走这里）。
# 否则 bin/start.js / bin/stop.js 的 `docker compose --env-file .env` 会立刻失败。
# secrets/ 暂留空（待运维手工跑 `make secrets` 替换 __CHANGE_ME__）。
if [ ! -f "${NEW_DIR}/.env" ] && [ -f "${NEW_DIR}/.env.example" ]; then
  cp -a "${NEW_DIR}/.env.example" "${NEW_DIR}/.env"
  echo "[swap] ✓ 从 .env.example 兜底 .env（占位符需后续跑 make secrets 替换）"
fi

# 3. 把当前 active 退到 staging-prev（rollback 锚点）
if [ "${SAME_DIR}" = "0" ] && [ -n "${PREV}" ]; then
  [ -L "${PREV_LINK}" ] && rm -f "${PREV_LINK}"
  ln -sfn "${PREV}" "${PREV_LINK}"
  echo "[swap] ✓ ${PREV_LINK} -> ${PREV}"
fi

# 4. 原子替换 staging-active
ln -sfn "${NEW_DIR}" "${ACTIVE_LINK}"
echo "[swap] ✓ ${ACTIVE_LINK} -> ${NEW_DIR}"

# 5. 重起容器
cd "${NEW_DIR}"
echo "[swap] → 停止旧容器..."
node bin/stop.js || true
echo "[swap] → 启动新容器..."
node bin/start.js --deploy

echo "[swap] ✓ active 已切到 ${NEW_SHA}（rollback: bash infrastructure/scripts/swap-staging-active.sh <old-sha>）"