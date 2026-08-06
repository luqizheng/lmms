#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# staging-gc.sh — 清理老的 staging-<sha> 目录与镜像
# 在 staging server 上由 cron.daily 触发，保留最近 N 个 SHA
#
# 用法（在 192.168.2.128 上）：
#   sudo install -m755 infrastructure/scripts/staging-gc.sh /etc/cron.daily/staging-gc
#   或手动跑：bash infrastructure/scripts/staging-gc.sh [--keep N] [--base <path>]
#
# 默认：
#   - 保留最近 5 个 staging-<sha>/
#   - docker image prune 72h 以上未用且 label=staging 的镜像
#   - 永不删 staging-active 和 staging-prev（即便不在最近 N）
# ═══════════════════════════════════════════════════════════════════════════════
set -euo pipefail

KEEP=5
BASE="${STAGING_BASE_DIR:-/home/kudi/coder-micro-services}"

for arg in "$@"; do
  case "${arg}" in
    --keep=*) KEEP="${arg#--keep=}" ;;
    --keep)   KEEP="${2:-5}"; shift ;;
    --base=*) BASE="${arg#--base=}" ;;
    --base)   BASE="${2:-${BASE}}"; shift ;;
    *) echo "[staging-gc] 未知参数: ${arg}" >&2; exit 2 ;;
  esac
done

ACTIVE_LINK="${BASE}/staging-active"
PREV_LINK="${BASE}/staging-prev"

# 计算 active + prev 的真实目录（防止被 GC 删）
PROTECTED=""
for link in "${ACTIVE_LINK}" "${PREV_LINK}"; do
  if [ -L "${link}" ]; then
    PROTECTED="${PROTECTED} $(readlink -f "${link}" 2>/dev/null || true)"
  fi
done

echo "[staging-gc] base=${BASE} keep=${KEEP}"
echo "[staging-gc] protected (永不删):${PROTECTED:- <none>}"

# 列出所有 staging-<sha>，按 mtime 倒序
all_dirs=$(ls -dt "${BASE}"/staging-* 2>/dev/null | grep -v 'staging-active$\|staging-prev$' || true)
if [ -z "${all_dirs}" ]; then
  echo "[staging-gc] 无 staging-<sha> 目录，退出"
  exit 0
fi

# 保留最近 N 个；其余删
echo "${all_dirs}" | awk -v keep="${KEEP}" 'NR>keep {print}' | while read -r dir; do
  [ -z "${dir}" ] && continue
  # 再次确认不在 protected 列表里
  skip=0
  for p in ${PROTECTED}; do
    [ "${dir}" = "${p}" ] && skip=1 && break
  done
  if [ "${skip}" = "1" ]; then
    echo "[staging-gc] ⊘ 跳过受保护目录: ${dir}"
    continue
  fi
  echo "[staging-gc] 🗑 rm -rf ${dir}"
  rm -rf "${dir}"
done

# 镜像清理（72h 未用 + 标记为 staging=true）
echo "[staging-gc] → docker image prune"
docker image prune -f \
  --filter "until=72h" \
  --filter "label=staging=true" 2>/dev/null || echo "[staging-gc] ⊘ prune 失败（可能没有 staging 标签的镜像）"

echo "[staging-gc] ✓ 完成"