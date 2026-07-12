#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# generate-secrets.sh — 生成 JWT 密钥对 + 随机密码，自动替换 .env 占位符
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SECRETS_DIR="${ROOT}/secrets"
ENV_FILE="${ROOT}/.env"

mkdir -p "${SECRETS_DIR}"

echo "[secrets] 生成 JWT RS256 密钥对..."
if [[ ! -f "${SECRETS_DIR}/jwt_private.pem" ]]; then
  openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 \
    -out "${SECRETS_DIR}/jwt_private.pem" 2>/dev/null
  openssl rsa -in "${SECRETS_DIR}/jwt_private.pem" -pubout \
    -out "${SECRETS_DIR}/jwt_public.pem" 2>/dev/null
  chmod 600 "${SECRETS_DIR}/jwt_private.pem"
  echo "[secrets] ✓ JWT 密钥对已生成"
else
  echo "[secrets] ✓ JWT 密钥对已存在，跳过"
fi

# 生成 8 个服务的 bootstrap token
declare -A TOKENS=(
  [IAM_DEV_BT]=iam
  [TOKEN_DEV_BT]=token
  [NOTIFICATION_DEV_BT]=notification
  [AUDIT_DEV_BT]=audit
  [PRUE_DEV_BT]=prue
  [GATEWAY_DEV_BT]=gateway
  [CONFIG_DEV_BT]=config
  [FILE_DEV_BT]=file
)

# 仅当 .env 中是 __CHANGE_ME__ 时才替换
replace_env() {
  local key="$1" val="$2"
  if [[ -f "${ENV_FILE}" ]] && grep -q "^${key}=__CHANGE_ME__" "${ENV_FILE}" 2>/dev/null; then
    sed -i "s|^${key}=__CHANGE_ME__$|${key}=${val}|" "${ENV_FILE}"
    echo "[secrets] ✓ ${key} 已替换"
  else
    echo "[secrets]   ${key} 未替换（非 __CHANGE_ME__，跳过）"
  fi
}

# 初始化 .env
if [[ ! -f "${ENV_FILE}" ]]; then
  cp "${ROOT}/.env.example" "${ENV_FILE}"
  echo "[secrets] ✓ .env 已从 .env.example 复制"
fi

echo "[secrets] 替换敏感占位符..."
replace_env MYSQL_ROOT_PASSWORD "$(openssl rand -hex 16)"
replace_env MYSQL_PASSWORD       "$(openssl rand -hex 16)"
replace_env REDIS_PASSWORD       "$(openssl rand -hex 16)"
replace_env RABBITMQ_DEFAULT_PASS "$(openssl rand -hex 16)"

for bt_key in "${!TOKENS[@]}"; do
  svc="${TOKENS[$bt_key]}"
  tok=$(openssl rand -hex 32)
  replace_env "${bt_key}" "${tok}"
done

echo ""
echo "[secrets] 完成。文件清单："
ls -la "${SECRETS_DIR}/"
echo ""
echo "⚠️  jwt_private.pem 已 chmod 600，请勿提交到 git"
echo "⚠️  运行 'make up' 开始启动服务"