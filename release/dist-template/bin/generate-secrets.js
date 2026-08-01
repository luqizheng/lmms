#!/usr/bin/env node
/**
 * generate-secrets.js — 生成 JWT 密钥对 + 随机密码，自动替换 .env 占位符
 * 等价于原 release/dist-template/bin/generate-secrets.sh
 */
const { execSync } = require('child_process');
const { existsSync, mkdirSync, copyFileSync, readFileSync, writeFileSync } = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const SECRETS_DIR = path.join(ROOT, 'secrets');
const ENV_FILE = path.join(ROOT, '.env');

mkdirSync(SECRETS_DIR, { recursive: true });

// ── 生成 JWT 密钥对 ───────────────────────────────────────────────────────────
console.log('[secrets] 生成 JWT RS256 密钥对...');
const jwtPrivate = path.join(SECRETS_DIR, 'jwt_private.pem');
const jwtPublic = path.join(SECRETS_DIR, 'jwt_public.pem');

if (!existsSync(jwtPrivate)) {
  execSync('openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "${jwtPrivate}"', {
    env: { ...process.env, jwtPrivate },
    stdio: 'pipe',
    shell: true,
  });
  execSync('openssl rsa -in "${jwtPrivate}" -pubout -out "${jwtPublic}"', {
    env: { ...process.env, jwtPrivate, jwtPublic },
    stdio: 'pipe',
    shell: true,
  });
  execSync(`chmod 600 "${jwtPrivate}"`, { stdio: 'pipe' });
  console.log('[secrets] ✓ JWT 密钥对已生成');
} else {
  console.log('[secrets] ✓ JWT 密钥对已存在，跳过');
}

// ── 初始化 .env ───────────────────────────────────────────────────────────────
if (!existsSync(ENV_FILE)) {
  copyFileSync(path.join(ROOT, '.env.example'), ENV_FILE);
  console.log('[secrets] ✓ .env 已从 .env.example 复制');
}

// ── 替换函数 ──────────────────────────────────────────────────────────────────
function replaceEnv(key, val) {
  const content = readFileSync(ENV_FILE, 'utf-8');
  const pattern = new RegExp(`^${key}=__CHANGE_ME__$`, 'm');
  if (pattern.test(content)) {
    const newContent = content.replace(pattern, `${key}=${val}`);
    writeFileSync(ENV_FILE, newContent);
    console.log(`[secrets] ✓ ${key} 已替换`);
  } else {
    console.log(`[secrets]   ${key} 未替换（非 __CHANGE_ME__，跳过）`);
  }
}

// ── 生成随机密码并替换 ───────────────────────────────────────────────────────
function randHex(len) {
  return execSync(`openssl rand -hex ${len}`, { stdio: 'pipe' }).toString().trim();
}

console.log('[secrets] 替换敏感占位符...');
replaceEnv('MYSQL_ROOT_PASSWORD', randHex(16));
replaceEnv('MYSQL_PASSWORD', randHex(16));
replaceEnv('REDIS_PASSWORD', randHex(16));
replaceEnv('RABBITMQ_DEFAULT_PASS', randHex(16));

// 服务 bootstrap token
const TOKENS = {
  IAM_DEV_BT: 'iam',
  TOKEN_DEV_BT: 'token',
  NOTIFICATION_DEV_BT: 'notification',
  AUDIT_DEV_BT: 'audit',
  PRUE_DEV_BT: 'prue',
  GATEWAY_DEV_BT: 'gateway',
  CONFIG_DEV_BT: 'config',
  FILE_DEV_BT: 'file',
};

for (const [btKey] of Object.entries(TOKENS)) {
  replaceEnv(btKey, randHex(32));
}

console.log('');
console.log('[secrets] 完成。文件清单：');
execSync(`ls -la "${SECRETS_DIR}/"`, { stdio: 'inherit' });
console.log('');
console.log('⚠️  jwt_private.pem 已 chmod 600，请勿提交到 git');
console.log("⚠️  运行 'node bin/start.js' 开始启动服务");
