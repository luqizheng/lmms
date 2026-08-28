#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# check-compose-commands.sh — 校验 docker-compose 的 command: 字段
#
# 背景：Cargo [[bin]] name 用 kebab-case（如 notification-service），
#       cargo build 强制把产物文件名转为下划线（如 notification_service）。
#       容器内 /usr/local/bin/<bin-name> 是下划线版。
#       因此 compose command: 字段必须用下划线版，否则容器启动时报
#       'executable file not found in $PATH'。
#       这个错误在下游 consumer 那边很常见，本仓 fix 后随 airgap 包
#       three-way-merge 覆盖到所有下游。
#
# 用法：
#   bash .agents/skills/coder-service-release/scripts/check-compose-commands.sh \
#     <compose-file>
#
# 输出：
#   OK   — 全部一致
#   FAIL — 列出不匹配的 service 与期望的下划线 binary 名
#
# 跨平台：Linux / macOS / Windows（Git Bash / WSL）。
# ─────────────────────────────────────────────────────────────────────────────

set -uo pipefail

COMPOSE_FILE="${1:-infrastructure/docker-compose-services.yml}"
# 4 级 ..: scripts -> skill -> skills -> .agents -> repo root
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"

if [[ ! -f "${COMPOSE_FILE}" ]]; then
  echo "[check-compose] ✗ compose 文件不存在: ${COMPOSE_FILE}" >&2
  exit 2
fi

# ── 颜色 ──────────────────────────────────────────────────────────────────
if [[ -t 1 ]]; then
  GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; NC='\033[0m'
else
  GREEN=''; RED=''; YELLOW=''; NC=''
fi

ERRORS=0
WARNINGS=0

# ── 1. 提取 compose 中的 services.<name>.command 列表 ─────────────────────
# 用 awk 简化解析（不依赖 yq / python）
# 状态机：
#   - 在 services: 块内
#   - 2 空格缩进行 = service 名
#   - 4 空格缩进行 = 该 service 的字段
#   - 匹配 `command:` 字段，记录命令字符串

# 用 awk 把每个 service 块压平：输出格式 "<service>|<command-line>"
# 然后 bash 处理 command 字符串（去掉 [ ] 和引号）
COMPOSE_SERVICES_WITH_CMD="$(
  awk '
    /^services:/ { flag=1; next }
    flag && /^[^ ]/ { flag=0 }
    flag && /^  [a-zA-Z0-9_-]+:/ {
      svc=$1; sub(/:$/, "", svc)
      current_svc=svc
    }
    flag && /^    command:/ {
      # 取整行（包括换行延续 — compose YAML 可写成 list of list）
      cmd=$0
      sub(/^    command:[ \t]*/, "", cmd)
      # 单行命令
      if (cmd != "") {
        print current_svc "|" cmd
      } else {
        # 多行 list，跳到下一行直到遇到非 - 开头的行（简化处理）
        in_cmd=1
        next
      }
    }
    flag && in_cmd && /^      -/ {
      cmd=$0
      sub(/^      -[ \t]*/, "", cmd)
      print current_svc "|" cmd
      next
    }
    flag && in_cmd && !/^      -/ {
      in_cmd=0
    }
  ' "${COMPOSE_FILE}"
)"

if [[ -z "${COMPOSE_SERVICES_WITH_CMD}" ]]; then
  echo "[check-compose] ! 未发现任何 command: 字段（compose 可能为空或仅用默认 ENTRYPOINT）"
  exit 0
fi

# ── 2. 解析每个 service 的 Cargo.toml，提取 binary name ──────────────────
# 优先 [[bin]] 块的 name；若没有 [[bin]] 块（默认 binary 推断），
# 用 [package] 的 name。两者都按 cargo 规则转为下划线文件名后再匹配 command:。
extract_bin_names() {
  local cargo_toml="$1"
  if grep -qE '^\[\[bin\]\]' "${cargo_toml}"; then
    awk '
      /^\[\[bin\]\]/ { in_bin=1; next }
      in_bin && /^\[/ { in_bin=0 }
      in_bin && /^name[ \t]*=/ {
        match($0, /"[^"]+"/)
        if (RLENGTH > 0) {
          bin = substr($0, RSTART+1, RLENGTH-2)
          print bin
        }
      }
    ' "${cargo_toml}"
  else
    awk '
      /^\[package\]/ { in_pkg=1; next }
      in_pkg && /^\[/ { in_pkg=0 }
      in_pkg && /^name[ \t]*=/ {
        match($0, /"[^"]+"/)
        if (RLENGTH > 0) {
          pkg = substr($0, RSTART+1, RLENGTH-2)
          print pkg
        }
      }
    ' "${cargo_toml}"
  fi
}

# 把 kebab 转下划线（与 cargo bin 文件名生成规则一致）
kebab_to_underscore() {
  echo "$1" | tr '-' '_'
}

# ── 3. 逐 service 校验 ───────────────────────────────────────────────────
echo "[check-compose] 扫描 $(basename "${COMPOSE_FILE}")..."

while IFS='|' read -r svc cmd_raw; do
  # 去掉 [ ] 和引号（支持 list 形式）
  cmd="$(echo "${cmd_raw}" | tr -d '[]' | tr -d '"' | tr -d "'" | awk '{$1=$1};1')"

  # 跳过空 command（容器走默认 ENTRYPOINT，不强制校验）
  if [[ -z "${cmd}" ]]; then
    continue
  fi

  # 找 compose service 对应的 Cargo.toml
  # 优先匹配 services/<svc>/Cargo.toml
  CARGO_TOML=""
  for candidate in "services/${svc}/Cargo.toml" \
                   "services/$(echo "${svc}" | tr '_' '-')/Cargo.toml"; do
    if [[ -f "${REPO_ROOT}/${candidate}" ]]; then
      CARGO_TOML="${REPO_ROOT}/${candidate}"
      break
    fi
  done

  # Fallback: outbox-publisher 类服务,binary 在主服务的 [[bin]] 里
  # 例: notification-outbox-publisher 的 bin name "ntf_outbox_publisher"
  #     定义在 services/notification-service/Cargo.toml 的 [bin] 块
  if [[ -z "${CARGO_TOML}" ]]; then
    MAIN_SVC="${svc%-outbox-publisher}"
    if [[ -n "${MAIN_SVC}" && "${MAIN_SVC}" != "${svc}" ]]; then
      for candidate in "services/${MAIN_SVC}/Cargo.toml" \
                       "services/${MAIN_SVC}-service/Cargo.toml"; do
        if [[ -f "${REPO_ROOT}/${candidate}" ]]; then
          CARGO_TOML="${REPO_ROOT}/${candidate}"
          break
        fi
      done
    fi
  fi

  if [[ -z "${CARGO_TOML}" ]]; then
    echo "[check-compose] ! ${svc}: 未找到 Cargo.toml（command=${cmd_raw}）— 跳过"
    WARNINGS=$((WARNINGS+1))
    continue
  fi

  # 取 [[bin]] name 列表
  BIN_NAMES="$(extract_bin_names "${CARGO_TOML}")"
  if [[ -z "${BIN_NAMES}" ]]; then
    echo "[check-compose] ! ${svc}: Cargo.toml 没有 [[bin]] 块"
    WARNINGS=$((WARNINGS+1))
    continue
  fi

  # 期望：cmd 应该等于某 [[bin]] name 的下划线版
  EXPECTED=""
  while IFS= read -r bin; do
    UND="$(kebab_to_underscore "${bin}")"
    if [[ "${cmd}" == "${UND}" ]]; then
      EXPECTED="matched(${bin})"
      break
    fi
  done <<< "${BIN_NAMES}"

  if [[ -z "${EXPECTED}" ]]; then
    # 列出可选下划线 binary 名
    UNDERSCORE_OPTIONS="$(while IFS= read -r bin; do
      echo -n " $(kebab_to_underscore "${bin}")"
    done <<< "${BIN_NAMES}" | sed 's/^ //')"
    printf "${RED}\xe2\x9c\x97${NC} [check-compose] %s: command=%s 不匹配任何 [[bin]] name 的下划线形式（候选:%s）\n" \
      "${svc}" "${cmd_raw}" "${UNDERSCORE_OPTIONS}" >&2 | sed 's/^/[check-compose] /' >&2
    ERRORS=$((ERRORS+1))
  else
    printf "${GREEN}\xe2\x9c\x93${NC} [check-compose] %s: command=%s \xe2\x86\x92 %s\n" \
      "${svc}" "${cmd_raw}" "${EXPECTED}"
  fi
done <<< "${COMPOSE_SERVICES_WITH_CMD}"

echo
if [[ ${ERRORS} -gt 0 ]]; then
  echo "[check-compose] ✗ ${ERRORS} 处不匹配，${WARNINGS} 处警告" >&2
  echo "[check-compose]   修复方法：compose 的 command: 字段必须用下划线形式（cargo 把 kebab-case bin name 转为下划线文件）" >&2
  echo "[check-compose]   例如：command: ['notification_service']（不是 'notification-service'）" >&2
  exit 1
fi

if [[ ${WARNINGS} -gt 0 ]]; then
  echo "[check-compose] ! ${WARNINGS} 处警告（无 ERROR），可以继续发布"
  exit 0
fi

echo "[check-compose] ✓ 全部 command: 字段与 Cargo [[bin]] name 匹配"
exit 0
