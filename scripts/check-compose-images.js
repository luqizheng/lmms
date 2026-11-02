#!/usr/bin/env node
/**
 * check-compose-images.js — 校验 compose 里的镜像引用展开后无退化形态
 *
 * 背景：compose 的 ${VAR} 未定义时**只发 warning 不报错**，拼接型变量
 * （如 ${kudi_repos}/api-gateway:${IMAGE_TAG}）会静默展开成
 * `/api-gateway:1.6.0` 这种非法镜像名，错误推迟到 `docker compose up`
 * 拉镜像时才暴露。本脚本把这类问题提前到 CI / 提交前拦下。
 *
 * 用法：
 *   node scripts/check-compose-images.js
 *   node scripts/check-compose-images.js --env-file <path>
 *   node scripts/check-compose-images.js --dir infrastructure
 *
 * 退出码：0 = 通过，1 = 发现退化形态
 */

const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const REPO_ROOT = path.join(__dirname, '..');

// ── 参数解析 ────────────────────────────────────────────────────────────────
let envFile = null;
let composeDir = 'infrastructure';
const args = process.argv.slice(2);
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === '--env-file') { envFile = args[++i]; continue; }
  if (a === '--dir') { composeDir = args[++i]; continue; }
  if (a.startsWith('--env-file=')) { envFile = a.slice('--env-file='.length); continue; }
  if (a.startsWith('--dir=')) { composeDir = a.slice('--dir='.length); continue; }
  console.error(`[check-compose-images] 未知参数: ${a}`);
  process.exit(2);
}

const dir = path.isAbsolute(composeDir) ? composeDir : path.join(REPO_ROOT, composeDir);
const composeFile = path.join(dir, 'docker-compose.yml');

if (!fs.existsSync(composeFile)) {
  console.error(`[check-compose-images] ✗ compose 文件不存在: ${composeFile}`);
  process.exit(1);
}

// ── 收集 compose 展开后的镜像名 ─────────────────────────────────────────────
const composeCmd = ['compose', '-f', composeFile, 'config'];
if (envFile) composeCmd.splice(1, 0, '--env-file', path.isAbsolute(envFile) ? envFile : path.join(REPO_ROOT, envFile));

let stdout;
let stderr = '';
try {
  stdout = execFileSync('docker', composeCmd, { encoding: 'utf-8', stdio: ['ignore', 'pipe', 'pipe'] });
} catch (e) {
  stdout = e.stdout || '';
  stderr = e.stderr || '';
  // compose 自身失败（如变量缺失导致 image 为空）——已属于问题，交给下面的检查判定
  if (!stdout) {
    console.error(`[check-compose-images] ✗ docker compose config 执行失败:`);
    console.error(stderr.trim() || e.message);
    process.exit(1);
  }
}

// ── 退化形态检查 ────────────────────────────────────────────────────────────
const problems = [];
const images = [];

for (const m of stdout.matchAll(/^[ \t]+image:[ \t]+(.+)$/gm)) {
  const raw = m[1].trim().replace(/^["']|["']$/g, '');
  if (!raw || raw.startsWith('#')) continue;
  images.push(raw);

  if (!raw) {
    problems.push('镜像名为空（变量未定义且无 :- 默认值）');
    continue;
  }
  if (raw.startsWith('/')) {
    problems.push(`镜像名以 "/" 开头（仓库前缀变量未定义）: ${raw}`);
  }
  if (raw.includes('${')) {
    problems.push(`镜像名含未解析变量: ${raw}`);
  }
  if (raw.endsWith(':latest')) {
    problems.push(`镜像 tag 为 :latest（未指定具体版本）: ${raw}`);
  }
}

// compose 的 warning 也算问题（变量未定义）
const unsetWarnings = [...stderr.matchAll(/The "([A-Za-z_][A-Za-z0-9_]*)" variable is not set/g)]
  .map(m => m[1]);
for (const v of [...new Set(unsetWarnings)]) {
  problems.push(`变量未定义（compose warning）: ${v}`);
}

// ── 输出 ────────────────────────────────────────────────────────────────────
console.log(`[check-compose-images] 展开 ${images.length} 个镜像引用（env-file: ${envFile || 'compose 自动加载'})`);

if (problems.length > 0) {
  console.error('[check-compose-images] ✗ 发现镜像引用退化形态:');
  for (const p of problems) console.error(`  - ${p}`);
  process.exit(1);
}

console.log('[check-compose-images] ✓ 镜像引用全部正常');
