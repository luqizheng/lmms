#!/usr/bin/env node
/**
 * check-doc-links.js — 文档链接完整性门禁
 *
 * 检查仓库 .md 文件中的 markdown 内联链接 [text](path) 与引用式链接 [text]: path，
 * 只对 resolve 后落在以下 canonical 文档根路径的做 stat 检查（P0-1 回归门禁）：
 *
 *   - docs/ai-context.md
 *   - docs/decisions/
 *   - docs/guides/
 *   - docs/runbooks/
 *   - docs/USER_MANUAL.md
 *   - docs/user-manual/         （已删除，防止回添引用）
 *
 * 排除：
 *   - http(s):// / mailto: / file: / #anchor
 *   - docs/superpowers/（正常存在的研发过程文档）
 *   - node_modules / target / .git / .vitepress/dist
 *   - release/dist-template/release（冻结发布快照）
 *
 * 用法：node scripts/check-doc-links.js
 * 退出码：0=全部通过, 1=有死链
 */

const fs = require('fs');
const path = require('path');

const REPO_ROOT = path.resolve(__dirname, '..');

// 受监控的文档路径（resolve 后落在此集合内才做检查）
const WATCHED_PREFIXES = [
  '/docs/ai-context.md',
  '/docs/decisions/',
  '/docs/guides/',
  '/docs/runbooks/',
  '/docs/USER_MANUAL.md',
  '/docs/user-manual/',
];

function isWatched(relPath) {
  const normalized = '/' + relPath.split(path.sep).join('/');
  for (const prefix of WATCHED_PREFIXES) {
    if (normalized === prefix || normalized.startsWith(prefix)) return true;
  }
  return false;
}

// —— 排除规则 ——
const EXCLUDE_DIR_NAMES = new Set(['node_modules', 'target', '.git', 'dist']);
const EXCLUDE_PREFIXES = [
  path.join(REPO_ROOT, '.vitepress', 'dist'),
  path.join(REPO_ROOT, 'release', 'dist-template', 'release'),
];

function shouldExclude(filePath) {
  const rel = path.relative(REPO_ROOT, filePath);
  const parts = rel.split(path.sep);
  if (parts.some(p => EXCLUDE_DIR_NAMES.has(p))) return true;
  return EXCLUDE_PREFIXES.some(p => filePath.startsWith(p + path.sep) || filePath === p);
}

let hasError = false;

function main() {
  // 收集所有 .md 文件
  const mdFiles = [];
  function walk(dir) {
    let entries;
    try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return; }
    for (const entry of entries) {
      const full = path.join(dir, entry.name);
      if (shouldExclude(full)) continue;
      if (entry.isDirectory()) walk(full);
      else if (entry.isFile() && entry.name.endsWith('.md')) mdFiles.push(full);
    }
  }
  walk(REPO_ROOT);

  // markdown 链接正则
  const INLINE_RE = /\[([^\]]*)\]\(([^)]+)\)/g;
  const REF_RE = /^\[([^\]]+)\]\s*:\s*(\S+)/gm;

  for (const filePath of mdFiles) {
    let content;
    try { content = fs.readFileSync(filePath, 'utf-8'); } catch { continue; }

    // 跳过 fence 区域
    const lines = content.split('\n');
    const inFenceAtLine = new Set();
    let inFence = false;
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].trimStart().startsWith('```')) inFence = !inFence;
      if (inFence) inFenceAtLine.add(i + 1);
    }

    // 扫描内联链接 [text](path)
    let m;
    while ((m = INLINE_RE.exec(content)) !== null) {
      const target = m[2].trim();
      if (/^(https?:\/\/|mailto:|file:)/.test(target)) continue;
      if (/^#/.test(target)) continue;
      const clean = target.split('#')[0].split('?')[0];
      if (!clean) continue;
      const lineNum = content.substring(0, m.index).split('\n').length;
      if (inFenceAtLine.has(lineNum)) continue;

      const dir = path.dirname(filePath);
      const resolved = path.resolve(dir, clean);
      if (!resolved.startsWith(REPO_ROOT)) continue;

      const rel = path.relative(REPO_ROOT, resolved);
      if (!isWatched(rel)) continue;

      if (fs.existsSync(resolved)) continue;
      if (!path.extname(resolved) && fs.existsSync(resolved + '.md')) continue;

      const relFile = path.relative(REPO_ROOT, filePath);
      console.error(`DEAD LINK: ${relFile}:${lineNum} → "${target}" (${rel})`);
      hasError = true;
    }

    // 扫描引用式链接 [text]: path
    REF_RE.lastIndex = 0;
    while ((m = REF_RE.exec(content)) !== null) {
      const target = m[2].trim();
      if (/^(https?:\/\/|mailto:|file:)/.test(target)) continue;
      if (/^#/.test(target)) continue;
      const clean = target.split('#')[0].split('?')[0];
      if (!clean) continue;
      const lineNum = content.substring(0, m.index).split('\n').length;
      if (inFenceAtLine.has(lineNum)) continue;

      const dir = path.dirname(filePath);
      const resolved = path.resolve(dir, clean);
      if (!resolved.startsWith(REPO_ROOT)) continue;

      const rel = path.relative(REPO_ROOT, resolved);
      if (!isWatched(rel)) continue;

      if (fs.existsSync(resolved)) continue;
      if (!path.extname(resolved) && fs.existsSync(resolved + '.md')) continue;

      const relFile = path.relative(REPO_ROOT, filePath);
      console.error(`DEAD LINK: ${relFile}:${lineNum} → "${target}" (${rel})`);
      hasError = true;
    }
  }

  if (hasError) {
    console.error('\n✖ 受监控的文档路径存在死链，修好后提交。');
    process.exit(1);
  } else {
    console.log('✔ 受监控的文档链接全部可达。');
    process.exit(0);
  }
}

main();
