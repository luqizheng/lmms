#!/usr/bin/env bash
# infrastructure/lmms-pg-dev/stop.sh —— 本目录部署单元的停止入口
# 用法：
#   ./stop.sh              # down（保留 volumes）
#   ./stop.sh --volumes    # 连 named volume 一起删（file-storage 清空，慎用）
# down 的定位依据是 project 名 + 服务清单，与启动时用的 overlay 片无关。
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT="lmms-pg-dev"
BASE="docker-compose-lmms"
DOWN_ARGS=()
for arg in "$@"; do
  case "$arg" in
    --volumes) DOWN_ARGS+=(--volumes) ;;
    --help|-h) grep '^#' "$0" | sed '1d;$d' | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) DOWN_ARGS+=("$arg") ;;
  esac
done

cd "$DIR"
echo "[stop:$PROJECT] down..."
docker compose -p "$PROJECT" -f "$BASE.yml" down ${DOWN_ARGS[@]+"${DOWN_ARGS[@]}"}
