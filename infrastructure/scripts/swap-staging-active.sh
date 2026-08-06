#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# swap-staging-active.sh — 原子交换 staging-active 软链指向新 SHA 版本
#
# 由 .gitlab-ci.yml 的 deploy:staging / e2e:staging 调用（在 staging server 上）
# 核心不变量：staging-active 是唯一可变状态，原子交换；其他 staging-<sha>/ 不可变。
#
# 用法：
#   bash infrastructure/scripts/swap-staging-active.sh <new-sha> [--dry-run]
#
# 行为：
#   1. 校验目标 staging-<new-sha>/ 目录存在
#   2. 复用上版的 secrets/ + .env（避免每次 reset 密钥）
#   3. 把当前 active（旧版）退到 staging-prev（rollback 锚点）
#   4. ln -sfn 原子替换 staging-active 软链到新 SHA（atomic on Linux）
#   5. 重起容器（~30s downtime，staging 接受）
#
# 回滚：
#   bash infrastructure/scripts/swap-staging-active.sh <old-sha>
#   （新版的 staging-<old-sha>/ 还在，下一次 swap 会自动复位）
# ═══════════════════════════════════════════════════════════════════════════════
set -euo pipefail

NEW_SHA="${1:-}"
DRY_RUN=0
for arg in "${@:2}"; do
  case "${arg}" in
    --dry-run) DRY_RUN=1 ;;
    *) echo "[swap] 未知参数: ${arg}" >&2; exit 2 ;;
  esac
done

[ -n "${NEW_SHA}" ] || { echo "[swap] usage: swap-staging-active.sh <sha> [--dry-run]" >&2; exit 1; }

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
  echo "[swap] ⚠ active 已指向 ${NEW_SHA}，无操作"
  exit 0
fi

if [ "${DRY_RUN}" = "1" ]; then
  echo "[swap] --dry-run: 将做"
  echo "[swap]   prev=${PREV:-<none>}"
  echo "[swap]   new=${NEW_DIR}"
  echo "[swap]   复用 prev secrets/ + .env 到 new"
  echo "[swap]   ln -sfn ${PREV:-<none>} ${PREV_LINK}"
  echo "[swap]   ln -sfn ${NEW_DIR} ${ACTIVE_LINK}"
  echo "[swap]   cd ${NEW_DIR} && node bin/stop.js && node bin/start.js --deploy"
  exit 0
fi

# 2. 复用 secrets + .env（避免 reset JWT key）
if [ -n "${PREV}" ] && [ -d "${PREV}" ]; then
  if [ -d "${PREV}/secrets" ]; then
    cp -a "${PREV}/secrets/." "${NEW_DIR}/secrets/" 2>/dev/null || true
    echo "[swap] ✓ 复用 secrets/ from prev"
  fi
  if [ -f "${PREV}/.env" ]; then
    cp -a "${PREV}/.env" "${NEW_DIR}/.env" 2>/dev/null || true
    echo "[swap] ✓ 复用 .env from prev"
  fi
fi

# 3. 把当前 active 退到 staging-prev（rollback 锚点）
if [ -n "${PREV}" ]; then
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