#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# sync-staging-images.sh — 把缺失的上游基础镜像补同步到 staging 内网 registry
#
# ╭─ 背景 ──────────────────────────────────────────────────────────────────╮
# │ staging 部署链 (deploy:staging) 通过 release/dist-template/bin/load-    │
# │ images.js --mode=pull 从内网 registry 拉镜像；任一缺失整个 deploy job   │
# │ fail (exit 1)。历史案例 (job 24913, commit c63679fc)：prom/alertmanager│
# │ :v0.27.0 在内网仓库缺失 → deploy:staging 红标。                       │
# ╰────────────────────────────────────────────────────────────────────────╯
#
# ╭─ 设计 ──────────────────────────────────────────────────────────────────╮
# │ • 不替代 load-images.js 的 hard fail 行为——业务服务镜像缺失仍 fail- │
# │   fast；本脚本只服务于「缺一个第三方基础镜像」这类基础设施同步问题。  │
# │ • 默认从 docker.io 拉，可换 UPSTREAM_REGISTRY。                       │
# │ • 幂等：先 docker manifest inspect 内网仓库，已在则跳过。              │
# │ • --check / --dry-run / --from-tar 三种模式覆盖 dry 验证 / 离线同步。 │
# ╰────────────────────────────────────────────────────────────────────────╯
#
# 用法：
#   # 检查：只看不推
#   bash sync-staging-images.sh --check < images.txt
#
#   # 标准同步：从 docker.io 拉 → tag → push 到 zhcoder 内网
#   echo "prom/alertmanager:v0.27.0" | bash sync-staging-images.sh
#   bash sync-staging-images.sh --images "prom/alertmanager:v0.27.0 mysql:8.0"
#
#   # 预览：只打印会做什么
#   bash sync-staging-images.sh --dry-run --images "prom/alertmanager:v0.27.0"
#
#   # 离线：staging 服务器不通 docker.io 时用本地 tarball 喂入
#   bash sync-staging-images.sh --from-tar ./alertmanager.tar \
#       --images "prom/alertmanager:v0.27.0"
#
# 镜像列表来源（任选其一）：
#   --images "<sp-separated>"   命令行直接传
#   stdin (每行一个，# 开头为注释)
#
# Env 覆盖：
#   STAGING_REGISTRY    内网 registry 主机:端口（默认 zhcoder-docker-registry.com:8000）
#   STAGING_BASE_PATH   内网 registry 基础路径（默认 official/，需带尾斜杠）
#   UPSTREAM_REGISTRY   上游 registry（默认 docker.io；可换其它内网镜像源）
# ═══════════════════════════════════════════════════════════════════════════════
set -euo pipefail

STAGING_REGISTRY="${STAGING_REGISTRY:-zhcoder-docker-registry.com:8000}"
STAGING_BASE_PATH="${STAGING_BASE_PATH:-official/}"
UPSTREAM_REGISTRY="${UPSTREAM_REGISTRY:-docker.io}"

MODE="sync"           # sync | check | dry-run
FROM_TAR=""
IMAGES=()

usage() {
  awk '
    /^# ═{5,}/ && banner++ >= 2 { exit }
    banner >= 1 { sub(/^# ?/, ""); print }
  ' "$0" | head -40
  exit "${1:-0}"
}

# ── arg parsing ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --check)    MODE="check" ;;
    --dry-run)  MODE="dry-run" ;;
    --from-tar) FROM_TAR="${2:-}"; shift ;;
    --images)
      shift
      # shellcheck disable=SC2206
      IMAGES=( ${1:-} )
      ;;
    -h|--help)  usage 0 ;;
    *)
      echo "[sync] 未知参数: $1" >&2
      usage 2
      ;;
  esac
  shift
done

# stdin 兜底
if [[ ${#IMAGES[@]} -eq 0 && ! -t 0 ]]; then
  while IFS= read -r line; do
    [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue
    IMAGES+=("$line")
  done
fi

if [[ ${#IMAGES[@]} -eq 0 ]]; then
  echo "[sync] ✗ 没有要同步的镜像（用 --images 传列表，或从 stdin 喂入）" >&2
  exit 1
fi

command -v docker >/dev/null 2>&1 || { echo "[sync] ✗ docker 未安装" >&2; exit 1; }

if [[ -n "$FROM_TAR" && ! -f "$FROM_TAR" ]]; then
  echo "[sync] ✗ --from-tar 文件不存在: $FROM_TAR" >&2
  exit 1
fi

# 去掉 STAGING_BASE_PATH 尾斜杠（统一处理）
BASE="${STAGING_BASE_PATH%/}/"

# ── 主流程 ──────────────────────────────────────────────────────────────
declare -i synced=0 skipped=0 failed=0
for img in "${IMAGES[@]}"; do
  # 拆分 registry 前缀（首段含 . 表示这是 registry domain，如 quay.io/foo/bar:v1）
  if [[ "$img" == *.*/* || "$img" == *.*:* ]]; then
    upstream_registry="${img%%/*}"
    image_name="${img#*/}"
    # 拼接成完整 upstream ref（docker daemon 仍按 upstream_registry 路由）
    upstream_ref="${upstream_registry}/${image_name}"
    # staging ref：去掉原 registry 前缀，拼到内网
    staging_ref="${STAGING_REGISTRY}/${BASE}${image_name}"
  elif [[ "$img" != */* ]]; then
    # library 镜像（无 / 段）：manifest inspect 需要显式 library/，但 staging 仓库里就是 official/<name>
    upstream_ref="${UPSTREAM_REGISTRY%/}/library/${img}"
    staging_ref="${STAGING_REGISTRY}/${BASE}${img}"
  else
    upstream_ref="${UPSTREAM_REGISTRY%/}/${img}"
    staging_ref="${STAGING_REGISTRY}/${BASE}${img}"
  fi

  echo ""
  echo "[sync] ─── ${img} ───"
  echo "  upstream: ${upstream_ref}"
  echo "  staging : ${staging_ref}"

  case "$MODE" in
    check)
      if docker manifest inspect "${staging_ref}" >/dev/null 2>&1; then
        echo "  status  : ✓ 已在 staging"
        skipped=$((skipped+1))
      else
        echo "  status  : ✗ 缺失"
        failed=$((failed+1))
      fi
      ;;
    dry-run)
      echo "  mode    : DRY-RUN（不实际拉/推）"
      echo "  action  : [2/3] docker pull ${upstream_ref}"
      echo "  action  : [3/3] docker tag + push ${staging_ref}"
      ;;
    sync)
      # 1. 幂等：staging 已有 → 跳过
      if docker manifest inspect "${staging_ref}" >/dev/null 2>&1; then
        echo "  [1/3] staging 已有镜像，跳过"
        skipped=$((skipped+1))
        continue
      fi

      # 2. 拉上游 / load tarball
      if [[ -n "$FROM_TAR" ]]; then
        echo "  [2/3] docker load -i ${FROM_TAR}"
        if ! docker load -i "$FROM_TAR"; then
          echo "  ✗ load 失败: $FROM_TAR" >&2
          failed=$((failed+1))
          continue
        fi
      else
        # 拉时用简化 ref（不带 registry 前缀），让 docker daemon 按默认 registry 解析
        echo "  [2/3] docker pull ${img}"
        if ! docker pull "$img"; then
          echo "  ✗ pull 失败: ${img}" >&2
          failed=$((failed+1))
          continue
        fi
      fi

      # 3. tag + push
      echo "  [3/3] docker tag + push"
      docker tag "$img" "$staging_ref"
      if docker push "$staging_ref"; then
        synced=$((synced+1))
        # digest 校验摘要（不阻塞）
        local_digest=$(docker inspect --format='{{index .RepoDigests 0}}' "$upstream_ref" 2>/dev/null || echo "n/a")
        echo "  ✓ 同步完成 (digest: ${local_digest})"
      else
        echo "  ✗ push 失败: ${staging_ref}" >&2
        failed=$((failed+1))
      fi
      ;;
  esac
done

echo ""
echo "[sync] ─── 汇总 ───"
echo "  同步: ${synced}  跳过: ${skipped}  失败: ${failed}"

[[ $failed -eq 0 ]] || exit 1