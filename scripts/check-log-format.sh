#!/usr/bin/env bash
# scripts/check-log-format.sh
#
# CI gate：确保所有 tracing subscriber / fmt layer 都是 JSON 格式（CRI JSON 解析）
#
# 背景：
#   - Promtail (infrastructure/config/loki/promtail-staging.yaml) 用
#     `cri: {}` + `json:` pipeline 解析 stdout。
#   - 若 fmt layer 不是 JSON（默认 pretty / compact），trace_id/tenant_id/user_id
#     拿不到 → Loki 反查失效 → E2E drill 失败 → incident 排查断链。
#   - 见 ADR-007 D2/D3、§8 关键约定、T1 commit f04fa8e。
#
# 用法：
#   bash scripts/check-log-format.sh
#   退出码 0=OK，非 0=FAIL
#
# 守门策略：
#   扫描 services/ + libs/ 下所有 *.rs，匹配 `tracing_subscriber::fmt::layer`
#   或 `fmt::layer(` 的声明行（要求 layer 后紧接左括号，避免误报
#   `fmt::MakeWriter` / `fmt::Display` 等 trait 引用）；该声明往后 8 行内必须
#   出现 ".json()" 调用，否则报错。
#   已注释行 (`//`) 跳过；`.json()` 的非 tracing 调用（reqwest / serde）不
#   影响判定，因为我们匹配的是声明行的上下文，而不是孤立的 `.json()`。

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ERRORS=0
CHECKED_FILES=0
DECLARATION_PATTERNS='tracing_subscriber::fmt::layer|fmt::layer[[:space:]]*\('  # 见上方注释
LOOKAHEAD_LINES=8

cd "$REPO_ROOT"

scan_file() {
  local file="$1"
  CHECKED_FILES=$((CHECKED_FILES + 1))

  local decl_lines
  decl_lines=$(grep -nE "$DECLARATION_PATTERNS" "$file" || true)
  [[ -z "$decl_lines" ]] && return 0

  while IFS= read -r match; do
    local lineno="${match%%:*}"
    local first_non_space="${match#*:}"
    if [[ "$first_non_space" =~ ^[[:space:]]*// ]]; then
      continue
    fi

    local context
    context=$(sed -n "${lineno},$((lineno + LOOKAHEAD_LINES - 1))p" "$file")
    if echo "$context" | grep -qE '\.json\s*\('; then
      continue
    fi

    echo "ERROR: $file:${lineno} — fmt layer without .json() within ${LOOKAHEAD_LINES} lines"
    echo "       (Promtail CRI-JSON 解析会失败，trace_id/tenant_id 拿不到)"
    echo "       行：$first_non_space"
    ERRORS=$((ERRORS + 1))
  done <<< "$decl_lines"
}

while IFS= read -r -d '' file; do
  scan_file "$file"
done < <(find services libs -type f -name '*.rs' -print0 2>/dev/null)

if [ "$ERRORS" -gt 0 ]; then
  echo ""
  echo "check-log-format.sh: FAIL — $ERRORS non-JSON tracing fmt layer(s) found (scanned $CHECKED_FILES files)"
  exit 1
fi

echo "check-log-format.sh: OK — scanned $CHECKED_FILES .rs files, all tracing fmt layers use .json()"
exit 0