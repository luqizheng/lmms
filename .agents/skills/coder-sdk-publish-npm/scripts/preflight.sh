#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# preflight.sh — coder-sdk-publish-npm 发布前预检
#
# 用法：
#   bash .agents/skills/coder-sdk-publish-npm/scripts/preflight.sh <next-version>
#
# 检查项（任一失败立即 exit 1）：
#   1. release/dist-tools/package.json 存在且为合法 JSON
#   2. name == "@digua/dist-tools"
#   3. publishConfig.registry == "http://npm.kudi.repos/"
#   4. 当前 version < next-version（不允许降版本）
#   5. 工作区干净（release/dist-tools/ 下无未提交改动）
#   6. npm CLI >= 9 在 PATH 上
#   7. http://npm.kudi.repos/ 可达（HEAD 请求，5s 超时）
#   8. ~/.npmrc 或 NPM_TOKEN/NPM_CONFIG_REGISTRY 中至少有一处声明了 npm.kudi.repos token
#
# 跨平台：Linux / macOS / Windows（Git Bash / WSL）。避免使用 BSD-only 或 GNU-only 选项。
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
PKG_JSON="${REPO_ROOT}/release/dist-tools/package.json"
REGISTRY="http://npm.kudi.repos/"

# ── 颜色输出（不支持时退化到无色） ─────────────────────────────────────────
if [[ -t 1 ]]; then
  GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; NC='\033[0m'
else
  GREEN=''; RED=''; YELLOW=''; NC=''
fi

ok()   { printf "${GREEN}\xe2\x9c\x93${NC} %s\n" "$*"; }
fail() { printf "${RED}\xe2\x9c\x97${NC} %s\n" "$*" >&2; exit 1; }
warn() { printf "${YELLOW}!${NC} %s\n" "$*"; }

# ── 参数校验 ────────────────────────────────────────────────────────────────
if [[ $# -ne 1 ]]; then
  echo "用法: $0 <next-version>" >&2
  echo "示例: $0 1.1.0" >&2
  exit 2
fi
NEXT_VERSION="$1"

# 简单 semver 校验（major.minor.patch[-prerelease][+build]）
if ! [[ "${NEXT_VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.+][0-9A-Za-z.-]+)?$ ]]; then
  fail "next-version 必须是合法 semver，例如 1.1.0 / 1.0.0-rc.1（收到: ${NEXT_VERSION}）"
fi

# ── 1. package.json 存在且合法 JSON ────────────────────────────────────────
[[ -f "${PKG_JSON}" ]] || fail "找不到 ${PKG_JSON}"
command -v node >/dev/null 2>&1 || fail "需要 node CLI 解析 package.json"
node -e "JSON.parse(require('fs').readFileSync('${PKG_JSON}','utf-8'))" \
  >/dev/null 2>&1 || fail "${PKG_JSON} 不是合法 JSON"
ok "package.json 存在且合法"

# ── 2. name == "@digua/dist-tools" ─────────────────────────────────────────
ACTUAL_NAME="$(node -e "console.log(require('${PKG_JSON}').name)")"
[[ "${ACTUAL_NAME}" == "@digua/dist-tools" ]] \
  || fail "package.json name=${ACTUAL_NAME}，预期 @digua/dist-tools"
ok "name == @digua/dist-tools"

# ── 3. publishConfig.registry == "http://npm.kudi.repos/" ─────────────────
ACTUAL_REG="$(node -e "
  const p = require('${PKG_JSON}');
  console.log((p.publishConfig && p.publishConfig.registry) || '');
")"
if [[ "${ACTUAL_REG}" != "${REGISTRY}" ]]; then
  fail "publishConfig.registry=${ACTUAL_REG:-<未设置>}，预期 ${REGISTRY}"
fi
ok "publishConfig.registry == ${REGISTRY}"

# ── 4. 当前 version < next-version ─────────────────────────────────────────
CUR_VERSION="$(node -e "console.log(require('${PKG_JSON}').version)")"
# 用 node 跑 semver 比较（避免 shell 内手撸比较器）
GE_RESULT="$(node -e "
  const a='${CUR_VERSION}', b='${NEXT_VERSION}';
  function p(v){return v.split(/[.+-]/).map(x=>/^\d+$/.test(x)?+x:x);}
  const [aa,bb]=[p(a),p(b)];
  const L=Math.max(aa.length,bb.length);
  for(let i=0;i<L;i++){
    const x=aa[i]||0, y=bb[i]||0;
    if(typeof x==='number' && typeof y==='number'){ if(x>y)return 'gt'; if(x<y)return 'lt'; }
    else { const s=String(x||''), t=String(y||''); if(s>t)return 'gt'; if(s<t)return 'lt'; }
  }
  return 'eq';
")"
if [[ "${GE_RESULT}" != "lt" ]]; then
  if [[ "${GE_RESULT}" == "eq" ]]; then
    fail "当前版本 ${CUR_VERSION} 与目标版本 ${NEXT_VERSION} 相同（需严格递增）"
  else
    fail "当前版本 ${CUR_VERSION} 大于目标版本 ${NEXT_VERSION}，不允许降版本"
  fi
fi
ok "当前 ${CUR_VERSION} < 目标 ${NEXT_VERSION}"

# ── 5. 工作区干净（release/dist-tools/ 下无未提交改动） ─────────────────────
if command -v git >/dev/null 2>&1 && git -C "${REPO_ROOT}" rev-parse --git-dir >/dev/null 2>&1; then
  if ! git -C "${REPO_ROOT}" diff --quiet -- release/dist-tools/ 2>/dev/null \
     || ! git -C "${REPO_ROOT}" diff --cached --quiet -- release/dist-tools/ 2>/dev/null; then
    fail "release/dist-tools/ 下有未提交改动，先 commit 再发布"
  fi
  ok "release/dist-tools/ 工作区干净"
else
  warn "非 git 仓库或 git 不可用，跳过工作区检查"
fi

# ── 6. npm CLI >= 9 ────────────────────────────────────────────────────────
command -v npm >/dev/null 2>&1 || fail "需要 npm CLI（在 PATH 上）"
NPM_VERSION_RAW="$(npm --version 2>/dev/null || echo 0)"
NPM_MAJOR="${NPM_VERSION_RAW%%.*}"
if [[ "${NPM_MAJOR}" -lt 9 ]]; then
  fail "npm 版本 ${NPM_VERSION_RAW} 过低（需要 >= 9），请 npm install -g npm@latest"
fi
ok "npm ${NPM_VERSION_RAW}（>= 9）"

# ── 7. registry 可达 ──────────────────────────────────────────────────────
if command -v curl >/dev/null 2>&1; then
  HTTP_CODE="$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 "${REGISTRY}" || echo 000)"
  if [[ "${HTTP_CODE}" =~ ^(2|3)[0-9]{2}$ ]] || [[ "${HTTP_CODE}" == "401" ]]; then
    ok "${REGISTRY} 可达（HTTP ${HTTP_CODE}）"
  else
    fail "${REGISTRY} 不可达（HTTP ${HTTP_CODE}）"
  fi
elif command -v wget >/dev/null 2>&1; then
  if wget -q -T 5 -O /dev/null "${REGISTRY}" 2>/dev/null; then
    ok "${REGISTRY} 可达"
  else
    warn "无法验证 ${REGISTRY} 可达性（无 curl/wget），请人工确认"
  fi
else
  warn "无 curl/wget，跳过 registry 可达性检查"
fi

# ── 8. auth token 检查 ─────────────────────────────────────────────────────
HAVE_AUTH=0
if [[ -f "${HOME}/.npmrc" ]] && grep -q 'npm.kudi.repos' "${HOME}/.npmrc" 2>/dev/null; then
  HAVE_AUTH=1
fi
if [[ -n "${NPM_TOKEN:-}" ]] || [[ -n "${NPM_CONFIG_REGISTRY:-}" ]]; then
  HAVE_AUTH=1
fi
if [[ "${HAVE_AUTH}" -eq 1 ]]; then
  ok "检测到 npm.kudi.repos 凭证配置（~/.npmrc 或环境变量）"
else
  warn "未检测到 npm.kudi.repos 凭证："
  echo "    在 ~/.npmrc 中加入："
  echo "        registry=http://npm.kudi.repos/"
  echo "        //npm.kudi.repos/:_authToken=\${NPM_TOKEN}"
  echo "    或设置环境变量："
  echo "        NPM_TOKEN=<your-token>"
  echo "    然后重新跑 preflight.sh"
  exit 1
fi

echo
ok "全部预检通过 — 可以发布 ${CUR_VERSION} → ${NEXT_VERSION}"
echo
echo "下一步："
echo "  npm version ${NEXT_VERSION} --prefix release/dist-tools --no-git-tag-version"
echo "  npm test --prefix release/dist-tools"
echo "  npm publish --prefix release/dist-tools --dry-run   # 强烈建议"
echo "  pnpm run sdk:publish:npm                            # 实际发布"
