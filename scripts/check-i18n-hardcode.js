#!/usr/bin/env node
//
// 静态扫描 frontend/src/views/**/*.vue，标记未走 \$t() / t() 直接出现在 template
// 或 script 里的中文硬编码。
//
// 配套 frontend/AGENTS.md 「测试规范」 + docs/plans/2026-07-09-i18n-hardcode-cleanup.md
// Phase 3：新增 view 不应含硬编码中文，应一律走 \$t('menus.xxx')。
//
// 两种模式：
//   默认（strict）：只查 git diff <base>...HEAD + 未跟踪 中的新增 view，CI fail。
//                  避免一次性 140+ 条历史债爆炸。
//   --all (report)：扫所有 view 列历史债，exit 0（仅报告，不 fail）。
//
// 检测启发式：
//   - 在 .vue 文件全文找连续 ≥2 个 CJK 字符（避免单字 false positive）
//   - 排除 $t(`...`) / t("...") 内的中文（已 i18n 化）
//   - 排除 pure-admin demo 目录（与 check-vitest-coverage 同白名单）
//   - 排除注释行（// / <!-- -->）和字符串字面量外的纯标识符（如 const xxx = 'a'）
//
// base ref 来源（strict 模式，按优先级降序）：
//   1. 命令行参数：node scripts/check-i18n-hardcode.js <base>
//   2. 环境变量 I18N_HARDCODE_BASE
//   3. 默认 main
//
const fs = require("node:fs");
const path = require("node:path");
const { execSync } = require("node:child_process");

const REPO_ROOT = path.resolve(__dirname, "..");
const VIEWS_DIR = "frontend/src/views";
const CJK_RE = /[\u3400-\u9fff]{2,}/g;
// 匹配 $t(`...`) / t("...") / t('...') / t(`...`) 整段以便从文件内容里扣除
const I18N_CALL_RE = /\b(?:t|\$t)\(\s*([`'""])(?:\\\\|\\\1|(?!\1)[\s\S])*?\1\s*(?:,[^)]*)?\)/g;

const REPORT_MODE = process.argv.includes("--all");

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

function* walkAllViews(dir) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const e of entries) {
    const full = path.join(dir, e.name);
    if (e.isDirectory()) {
      // 子目录的 index.vue 即为该 view；否则继续递归
      const idx = path.join(full, "index.vue");
      if (fs.existsSync(idx)) {
        yield idx;
      } else {
        yield* walkAllViews(full);
      }
    } else if (
      e.isFile() &&
      e.name.endsWith(".vue") &&
      e.name !== "index.vue"
    ) {
      yield full;
    }
  }
}

function getBaseRef() {
  const arg = process.argv[2];
  if (arg) return arg;
  if (process.env.I18N_HARDCODE_BASE) return process.env.I18N_HARDCODE_BASE;
  return "main";
}

function gitNewFiles(base) {
  try {
    execSync("git rev-parse --git-dir", { cwd: REPO_ROOT, stdio: "ignore" });
  } catch {
    console.error("x not a git repo");
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
      console.error(
        `x git query failed (${q.split(" ")[1] || q}): ${e.message.split("\n")[0]}`
      );
      return null;
    }
  }
  return [...collected];
}

// 从 .vue 文件内容里剥离 i18n 调用，再找出剩余的 CJK 段
function findHardcode(content) {
  // 1) 删注释行（// 和 /* */ 单行）
  const stripped = content
    .replace(/\/\/[^\n]*/g, "")
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/<!--[\s\S]*?-->/g, "");
  // 2) 删 $t(`...`) / t("...") 整段
  const noI18n = stripped.replace(I18N_CALL_RE, "");
  // 3) 找剩余 CJK
  const findings = [];
  const lines = noI18n.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const matches = lines[i].match(CJK_RE);
    if (matches) {
      findings.push({ line: i + 1, samples: matches.slice(0, 3) });
    }
  }
  return findings;
}

function checkFile(absPath) {
  const content = fs.readFileSync(absPath, "utf8");
  return findHardcode(content);
}

const base = getBaseRef();

if (REPORT_MODE) {
  // --all 模式：扫所有 view 列历史债
  const viewsDirAbs = path.join(REPO_ROOT, VIEWS_DIR);
  const all = [];
  for (const v of walkAllViews(viewsDirAbs)) {
    const rel = path.relative(REPO_ROOT, v);
    if (!isPureAdminDemo(rel)) all.push(rel);
  }
  const withHardcode = [];
  for (const rel of all) {
    const findings = checkFile(path.join(REPO_ROOT, rel));
    if (findings.length > 0) {
      withHardcode.push({ rel, findings });
    }
  }
  console.log("");
  console.log(`i18n hardcode report (excluding pure-admin demo dirs)`);
  console.log(`  total views       : ${all.length}`);
  console.log(`  views w/ hardcode : ${withHardcode.length}`);
  console.log(`  total findings    : ${withHardcode.reduce((s, x) => s + x.findings.length, 0)}`);
  if (withHardcode.length > 0) {
    console.log("");
    console.log("  hardcoded views (debt):");
    for (const { rel, findings } of withHardcode) {
      console.log(`    - ${rel} (${findings.length} occurrences)`);
    }
  }
  console.log("");
  process.exit(0);
}

const newFiles = gitNewFiles(base);
if (newFiles === null) process.exit(1);

const newViews = newFiles.filter(
  (f) =>
    f.startsWith(VIEWS_DIR + "/") &&
    f.endsWith(".vue") &&
    !isPureAdminDemo(f)
);

const findings = [];
const deleted = [];
for (const rel of newViews) {
  const abs = path.join(REPO_ROOT, rel);
  if (!fs.existsSync(abs)) {
    deleted.push(rel);
    continue;
  }
  const fileFindings = checkFile(abs);
  if (fileFindings.length > 0) {
    findings.push({ rel, fileFindings });
  }
}

console.log("");
if (newViews.length === 0) {
  console.log(`v no new .vue files in ${VIEWS_DIR} (vs ${base})`);
  process.exit(0);
}

if (findings.length > 0) {
  for (const { rel, fileFindings } of findings) {
    console.log(`x ${rel}`);
    for (const f of fileFindings) {
      const sample = f.samples.join(" / ");
      console.log(`    L${f.line}: ${sample.slice(0, 60)}`);
    }
  }
  console.log("");
  console.log(`Scanned ${newViews.length} new .vue file(s) (vs ${base})`);
  console.log(
    `x ${findings.length} new view(s) contain hardcoded Chinese`
  );
  console.log("    fix: 走 \$t('menus.xxx')，或在白名单（pure-admin demo）");
  if (deleted.length > 0) {
    console.log(
      `    (${deleted.length} new .vue file(s) already deleted from working tree, skipped)`
    );
  }
  process.exit(1);
}

console.log(`Scanned ${newViews.length} new .vue file(s) (vs ${base})`);
console.log(`v all new views are free of hardcoded Chinese`);
if (deleted.length > 0) {
  console.log(
    `   (${deleted.length} new .vue file(s) already deleted from working tree, skipped)`
  );
}
process.exit(0);