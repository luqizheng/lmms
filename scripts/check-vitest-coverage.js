#!/usr/bin/env node
/**
 * 静态扫描 git diff <base>...HEAD + 未跟踪文件 中的 frontend/src/views/ 下的新增 .vue。
 * 验证每个新增 view 在同目录或任意子目录有 test 文件（.test.ts / .test.tsx）伴随。
 *
 * 配套 frontend/AGENTS.md「测试规范」强制：任一新增 src/views/ 下的 view
 * 至少交付一条 happy-path 单测。
 *
 * 只对新增 view 报错，避免一次性 140+ 条历史债爆炸（按"护航未来变更"思路）。
 *
 * base ref 来源（按优先级降序）：
 *   1. 命令行参数：node scripts/check-vitest-coverage.js <base>
 *   2. 环境变量 VITEST_COVERAGE_BASE
 *   3. 默认 main
 *
 * 包含三类文件：
 *   - git diff --diff-filter=A base...HEAD：已提交的新增（CI 走 origin/main）
 *   - git diff --diff-filter=A base：工作区新增（含未跟踪，本地开发用）
 *   - git ls-files --others --exclude-standard：未跟踪文件
 */

const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

const REPO_ROOT = path.resolve(__dirname, "..");
const VIEWS_DIR = "frontend/src/views";
const TEST_GLOB = /\.test\.(ts|tsx)$/;

const PURE_ADMIN_DEMO_DIRS = [
  "about",
  "able",
  "empty",
  "error",
  "guide",
  "list",
  "menuoverflow",
  "nested",
  "result",
  "table",
  "tabs"
];

function isPureAdminDemo(relPath) {
  const rel = relPath.startsWith(VIEWS_DIR + "/")
    ? relPath.slice(VIEWS_DIR.length + 1)
    : relPath;
  const first = rel.split(path.sep)[0];
  return PURE_ADMIN_DEMO_DIRS.includes(first);
}

function getBaseRef() {
  const arg = process.argv[2];
  if (arg) return arg;
  if (process.env.VITEST_COVERAGE_BASE) return process.env.VITEST_COVERAGE_BASE;
  return "main";
}

function gitNewFiles(base) {
  try {
    execSync("git rev-parse --git-dir", { cwd: REPO_ROOT, stdio: "ignore" });
  } catch {
    console.error("✗ not a git repo");
    return null;
  }
  const collected = new Set();
  const queries = [
    `git diff --name-only --diff-filter=A ${base}...HEAD`,
    `git diff --name-only --diff-filter=A ${base}`,
    `git ls-files --others --exclude-standard`
  ];
  for (const q of queries) {
    try {
      const out = execSync(q, { cwd: REPO_ROOT, encoding: "utf8" });
      for (const f of out.split("\n")) {
        if (f) collected.add(f);
      }
    } catch (e) {
      console.error(`✗ git query failed (${q.split(" ")[1] || q}): ${e.message.split("\n")[0]}`);
      return null;
    }
  }
  return [...collected];
}

function* walkForTests(dir) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const e of entries) {
    const full = path.join(dir, e.name);
    if (e.isDirectory()) yield* walkForTests(full);
    else if (TEST_GLOB.test(e.name)) yield full;
  }
}

function hasAdjacentTest(viewAbsPath) {
  const dir = path.dirname(viewAbsPath);
  for (const _ of walkForTests(dir)) return true;
  return false;
}

const base = getBaseRef();
const newFiles = gitNewFiles(base);
if (newFiles === null) process.exit(1);

const newViews = newFiles.filter(
  f =>
    f.startsWith(VIEWS_DIR + "/") &&
    f.endsWith(".vue") &&
    !isPureAdminDemo(f)
);

const missing = [];
const deleted = [];
for (const rel of newViews) {
  const abs = path.join(REPO_ROOT, rel);
  if (!fs.existsSync(abs)) {
    deleted.push(rel);
    continue;
  }
  if (!hasAdjacentTest(abs)) {
    missing.push(rel);
  }
}

console.log("");
if (newViews.length === 0) {
  console.log(`✓ no new .vue files in ${VIEWS_DIR} (vs ${base})`);
  process.exit(0);
}

if (missing.length > 0) {
  for (const f of missing) {
    console.log(`✗ ${f}: 缺单测（no *.test.{ts,tsx} in ${path.dirname(f)}/）`);
  }
  console.log("");
  console.log(`Scanned ${newViews.length} new .vue file(s) (vs ${base})`);
  console.log(`❌ ${missing.length} view(s) without adjacent test`);
  console.log("    fix: 加 <name>.test.ts 同目录，或 <sub>/<adapter>.test.ts 子目录");
  if (deleted.length > 0) {
    console.log(`    (${deleted.length} new .vue file(s) already deleted from working tree, skipped)`);
  }
  process.exit(1);
}

console.log(`Scanned ${newViews.length} new .vue file(s) (vs ${base})`);
console.log(`✅ all new views have adjacent test coverage`);
if (deleted.length > 0) {
  console.log(`   (${deleted.length} new .vue file(s) already deleted from working tree, skipped)`);
}
process.exit(0);
