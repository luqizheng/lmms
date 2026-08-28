#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# preflight.sh — coder-sdk-publish-maven 发布前预检
#
# 用法：
#   bash .agents/skills/coder-sdk-publish-maven/scripts/preflight.sh \
#     <release|snapshot> <version>
#
# 检查项（任一 ERROR 立即 exit 1）：
#   1. mvn --version 在 PATH
#   2. curl --version 在 PATH（默认 upload mode 用）
#   3. java --version 在 PATH + JAVA_HOME（可选但推荐）
#   4. sdk/java/pom.xml 是合法 XML 且 groupId == io.digua
#   5. 7 个 module 都存在且各自有 pom.xml（除非 --modules 过滤）
#   6. <version> 格式与 release/snapshot 模式匹配
#   7. Reposilite base URL HEAD 可达
#   8. 至少一个 credential source 可用（CLI / env / wizard / ~/.m2/settings.xml）
#   9. 当前 sdk/java/ 工作区状态（uncommitted drift 警告）
#
# 跨平台：Linux / macOS / Windows（Git Bash / WSL）。避免 BSD-only / GNU-only 选项。
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
REPO_ROOT="$(cd "${SKILL_DIR}/../.." && pwd)"

POM_FILE="${REPO_ROOT}/sdk/java/pom.xml"
WIZARD_CONFIG="${REPO_ROOT}/infrastructure/.docker-registry.json"
M2_SETTINGS="${HOME}/.m2/settings.xml"

# 默认 base（与 pom.xml <distributionManagement> + maven-publish.js DEFAULT 一致）
REPOSILITE_BASE="http://192.168.2.12:18181"

# pom.xml 列出的 modules（与 sdk/java/pom.xml 第 13-21 行对齐）
EXPECTED_MODULES=(
  common
  digua-user-user-sdk
  digua-user-client-sdk
  digua-token-user-sdk
  digua-token-client-sdk
  digua-http-client
  digua-notification-sdk
)

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

# ── 参数校验 ──────────────────────────────────────────────────────────────
if [[ $# -ne 2 ]]; then
  echo "用法: $0 <release|snapshot> <version>" >&2
  echo "示例: $0 release 0.4.0" >&2
  echo "      $0 snapshot 0.4.1-SNAPSHOT" >&2
  exit 2
fi
MODE="$1"
VERSION="$2"

if [[ "${MODE}" != "release" && "${MODE}" != "snapshot" ]]; then
  fail "mode 必须是 release / snapshot（收到: ${MODE}）"
fi

if ! [[ "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.-]+)?$ ]]; then
  fail "version 必须是合法 semver（收到: ${VERSION}）"
fi

# 强制 -SNAPSHOT 互斥
if [[ "${MODE}" == "release" ]] && [[ "${VERSION}" == *-SNAPSHOT ]]; then
  fail "release 模式禁止 -SNAPSHOT 版本（Reposilite releases 仓库拒收）"
fi
if [[ "${MODE}" == "snapshot" ]] && [[ "${VERSION}" != *-SNAPSHOT ]]; then
  fail "snapshot 模式必须带 -SNAPSHOT 后缀（Reposilite snapshots 仓库拒收非 SNAPSHOT）"
  info "建议：${VERSION}-SNAPSHOT"
fi

echo
echo "============================================"
echo " Maven SDK Publish Pre-flight"
echo " Mode:     ${MODE}"
echo " Version:  ${VERSION}"
echo " Registry: Reposilite @ ${REPOSILITE_BASE}"
echo "============================================"
echo

# ── 1. mvn / curl / java ────────────────────────────────────────────────
command -v mvn >/dev/null 2>&1 || fail "需要 mvn CLI（在 PATH 上）"
MVN_VERSION_RAW="$(mvn --version 2>&1 | head -1 | awk '{print $3}')"
MVN_MAJOR="${MVN_VERSION_RAW%%.*}"
if [[ "${MVN_MAJOR}" -lt 3 ]]; then
  fail "mvn 版本 ${MVN_VERSION_RAW} 过低（需要 >= 3.9），请升级 Maven"
fi
ok "mvn ${MVN_VERSION_RAW}（>= 3.x）"

command -v curl >/dev/null 2>&1 || fail "需要 curl CLI（默认 upload mode 强制要求）"
ok "curl: $(curl --version | head -1)"

# Java 检查（soft）
if command -v java >/dev/null 2>&1; then
  JAVA_VERSION_RAW="$(java --version 2>&1 | head -1)"
  ok "java: ${JAVA_VERSION_RAW}"
else
  warn "java 不在 PATH 上，mvn 内部使用 JAVA_HOME；如果 mvn 可用则忽略"
fi

# ── 2. sdk/java/pom.xml 合法性 ──────────────────────────────────────────
[[ -f "${POM_FILE}" ]] || fail "找不到 ${POM_FILE}"
GROUP_ID="$(node -e "
  const fs = require('fs');
  const xml = fs.readFileSync('${POM_FILE}', 'utf-8');
  const m = /<groupId>\s*([^<]+?)\s*<\/groupId>/.exec(xml);
  console.log(m ? m[1].trim() : '');
")"
[[ "${GROUP_ID}" == "io.digua" ]] || fail "pom.xml groupId=${GROUP_ID:-<未找到>}，预期 io.digua"
ok "pom.xml groupId == io.digua"

CUR_POM_VERSION="$(node -e "
  const fs = require('fs');
  const xml = fs.readFileSync('${POM_FILE}', 'utf-8');
  const block = xml.match(/<project\b[\s\S]*?<\/project>/)[0].replace(/<parent>[\s\S]*?<\/parent>/g, '');
  const m = /<version>\s*([^<]+?)\s*<\/version>/.exec(block);
  console.log(m ? m[1].trim() : '');
")"
if [[ -z "${CUR_POM_VERSION}" ]]; then
  fail "无法从 pom.xml 解析 <version>"
fi
if [[ "${CUR_POM_VERSION}" == "${VERSION}" ]]; then
  ok "pom.xml 当前 <version> 已为 ${VERSION}（无需 bump）"
else
  ok "pom.xml 当前 ${CUR_POM_VERSION} → 目标 ${VERSION}（publish 脚本会自动 mvn versions:set）"
fi

# ── 3. 7 个 module 存在 ────────────────────────────────────────────────
echo
echo "[modules] -> 检查 ${#EXPECTED_MODULES[@]} 个 module..."
MISSING=0
for m in "${EXPECTED_MODULES[@]}"; do
  if [[ -f "${REPO_ROOT}/sdk/java/${m}/pom.xml" ]]; then
    ok "  sdk/java/${m}/pom.xml"
  else
    warn "  sdk/java/${m}/pom.xml 不存在（README 可能列了但 pom.xml 没启用）"
    MISSING=$((MISSING+1))
  fi
done
if [[ ${MISSING} -gt 0 ]]; then
  warn "${MISSING} 个 module 缺 pom.xml（可能是历史遗留或未启用，请人工确认）"
  info "  提示：pom.xml <modules> 是权威清单；README 中的模块名可能滞后"
fi

# ── 4. Reposilite base URL 可达 ─────────────────────────────────────────
echo
if command -v curl >/dev/null 2>&1; then
  HTTP_CODE="$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 "${REPOSILITE_BASE}/" || echo 000)"
  if [[ "${HTTP_CODE}" =~ ^(2|3)[0-9]{2}$ ]] || [[ "${HTTP_CODE}" == "401" ]] || [[ "${HTTP_CODE}" == "403" ]]; then
    ok "${REPOSILITE_BASE}/ 可达（HTTP ${HTTP_CODE}）"
  else
    fail "${REPOSILITE_BASE}/ 不可达（HTTP ${HTTP_CODE}）。检查网络或 VPN"
  fi
else
  warn "无 curl，跳过 Reposilite 可达性检查"
fi

# ── 5. credential source 检查 ──────────────────────────────────────────
echo
echo "[auth] -> 检查 credential source..."
HAVE_AUTH=0
SOURCE_LABEL=""

if [[ -n "${MAVEN_USERNAME:-}" && -n "${MAVEN_PASSWORD:-}" ]]; then
  HAVE_AUTH=1
  SOURCE_LABEL="env MAVEN_USERNAME/MAVEN_PASSWORD"
fi

if [[ ${HAVE_AUTH} -eq 0 && -f "${WIZARD_CONFIG}" ]]; then
  AUTH_MODE="$(node -e "
    try {
      const c = JSON.parse(require('fs').readFileSync('${WIZARD_CONFIG}','utf-8'));
      console.log(c.mavenAuthMode || 'none');
    } catch(e) { console.log('none'); }
  ")"
  if [[ "${AUTH_MODE}" == "userpass" ]] || [[ "${AUTH_MODE}" == "token" ]]; then
    HAVE_AUTH=1
    SOURCE_LABEL="wizard config (${AUTH_MODE})"
  fi
fi

if [[ ${HAVE_AUTH} -eq 0 && -f "${M2_SETTINGS}" ]]; then
  if grep -qE 'reposilite-repository-(releases|snapshots)' "${M2_SETTINGS}" 2>/dev/null; then
    HAVE_AUTH=1
    SOURCE_LABEL="~/.m2/settings.xml"
  fi
fi

if [[ ${HAVE_AUTH} -eq 1 ]]; then
  ok "credential source: ${SOURCE_LABEL}"
else
  fail "未找到任何 credential source。可选方式：
    1. CLI 参数: --user <user> --pass <pass>
    2. 环境变量: export MAVEN_USERNAME=<u> MAVEN_PASSWORD=<p>
    3. wizard config: pnpm run setup:wizard
    4. ~/.m2/settings.xml: 加 <server><id>reposilite-repository-releases</id>...</server>"
fi

# ── 6. git 工作区状态 ──────────────────────────────────────────────────
echo
if command -v git >/dev/null 2>&1 && git -C "${REPO_ROOT}" rev-parse --git-dir >/dev/null 2>&1; then
  if ! git -C "${REPO_ROOT}" diff --quiet -- sdk/java/ 2>/dev/null \
     || ! git -C "${REPO_ROOT}" diff --cached --quiet -- sdk/java/ 2>/dev/null; then
    warn "sdk/java/ 下有未提交改动（可能是上一次的 pom.xml 版本 bump 没 commit）"
    warn "  建议：git diff sdk/java/ && git add sdk/java && git commit -m 'feat(sdk): bump to <ver>'"
  else
    ok "sdk/java/ 工作区干净"
  fi
else
  warn "非 git 仓库，跳过工作区检查"
fi

# ── 总结 ────────────────────────────────────────────────────────────────
echo
ok "全部预检通过 — 可以 ${MODE} ${VERSION}"
echo
echo "下一步："
if [[ "${MODE}" == "snapshot" ]]; then
  echo "  pnpm run sdk:test:java                              # 跑测试（默认 java8 profile）"
  echo "  pnpm run sdk:publish:snapshot --version ${VERSION} --dry-run --profile java17"
  echo "  pnpm run sdk:publish:snapshot --version ${VERSION} --profile java17"
  echo "  git diff sdk/java/ && git add sdk/java && git commit -m 'feat(sdk): bump to ${VERSION}'"
else
  echo "  pnpm run sdk:test:java"
  echo "  pnpm run sdk:publish:release --version ${VERSION} --dry-run"
  echo "  pnpm run sdk:publish:release --version ${VERSION}"
  echo "  git add sdk/java && git commit -m 'feat(sdk): bump to ${VERSION}'"
  echo "  git tag v${VERSION}  # CI 会基于 tag 自动重跑发布"
fi
