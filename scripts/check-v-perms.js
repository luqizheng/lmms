#!/usr/bin/env node
/* eslint-disable */
/**
 * 静态扫描 frontend/src/views/{glob}.vue，标记未挂 v-perms / hasPerms 的写操作按钮。
 *
 * 启发式（正则 + 上下文）：
 *   1. 文件以 <script setup> 开头、含 import { hasPerms } from "@/utils/auth"
 *      或 v-perms 指令 / cellRenderer 内有 hasPerms(...) 调用 —— 视为已覆盖。
 *   2. el-button 出现且 type ∈ { primary, success, warning, danger }
 *      且 onclick / @click 调 createXxx / updateXxx / deleteXxx / removeXxx / assignXxx / revokeXxx / onDelete / onCreate
 *      或 el-button 文本含「新建/删除/修改/分配/踢/导出/提交/保存」
 *      且不在 v-if="hasPerms(...)" 块内，且没标 v-perms —— 报告。
 *   3. cellRenderer 内 JSX 写 el-button 无 hasPerms(...) 包裹 —— 报告。
 *
 * 误报率高（毕竟只是 regex），但作为 PR-check 门槛够用：未挂 perm 的写按钮会被高亮。
 * 豁免目录：views/login、views/error、views/welcome、views/guide、views/register、views/home
 */

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "../frontend/src/views");

const WRITE_KEYWORDS = [
  "create", "update", "delete", "remove", "assign", "revoke", "post",
  "onDelete", "onCreate", "onUpdate", "onSubmit", "onAssign",
  "onExport", "onCopy"
];

const TEXT_KEYWORDS = ["新建", "删除", "修改", "分配", "踢", "导出", "提交", "保存", "撤稿"];

const EXEMPT_DIRS = [
  "login",
  "error",
  "welcome",
  "guide",
  "register",
  "home",
  "able",
  "account-settings",
  "result"
];

function* walk(dir) {
  for (const f of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, f.name);
    if (f.isDirectory()) yield* walk(full);
    else if (f.name.endsWith(".vue")) yield full;
  }
}

function hasAuthImport(content) {
  return /from\s+["']@\/utils\/auth["']/.test(content);
}

function lineHit(content, regex) {
  const m = content.match(regex);
  return m ? m.index : -1;
}

function check(file) {
  const rel = path.relative(ROOT, file);
  const parts = rel.split(path.sep);
  if (parts.some(p => EXEMPT_DIRS.includes(p))) return null;

  const content = fs.readFileSync(file, "utf8");
  const auth = hasAuthImport(content);
  const lines = content.split("\n");

  const findings = [];

  lines.forEach((line, idx) => {
    const lineno = idx + 1;
    if (!/<el-button\b/.test(line)) return;
    if (/\bv-perms=/.test(line)) return;
    if (/v-if="hasPerms\(/.test(line)) return;
    if (/hasPerms\(/.test(line)) return;

    const isWrite = WRITE_KEYWORDS.some(kw => new RegExp(`\\b${kw}\\b`, "i").test(line));
    const isWriteText = TEXT_KEYWORDS.some(kw => line.includes(kw));
    if (!isWrite && !isWriteText) return;

    findings.push({
      file: rel,
      line: lineno,
      snippet: line.trim().slice(0, 100)
    });
  });

  // cellRenderer 块：检查 .tsx 配对文件中的按钮
  const tsxFile = file.replace(/\.vue$/, ".ts");
  const tsxAdjacent = file.replace(/\.vue$/, ".tsx");
  for (const t of [tsxFile, tsxAdjacent]) {
    if (!fs.existsSync(t)) continue;
    const c = fs.readFileSync(t, "utf8");
    const btnMatches = [...c.matchAll(/<el-button\b[^>]*onClick=/g)];
    for (const m of btnMatches) {
      const lineno = c.slice(0, m.index).split("\n").length;
      const snippet = c.split("\n")[lineno - 1]?.trim() ?? "";
      if (/hasPerms\(/.test(snippet)) continue;
      findings.push({
        file: path.relative(path.resolve(__dirname, "../frontend/src"), t),
        line: lineno,
        snippet: snippet.slice(0, 100)
      });
    }
  }

  if (!auth && findings.length > 0) {
    // 文件也没引 auth —— 整个文件级的写按钮都裸奔
  }
  return findings;
}

let totalFindings = 0;
const filesScanned = [];

for (const f of walk(ROOT)) {
  filesScanned.push(f);
  const fnd = check(f);
  if (fnd && fnd.length) {
    totalFindings += fnd.length;
    for (const r of fnd) {
      console.log(`✗ ${r.file}:${r.line}  ${r.snippet}`);
    }
  }
}

console.log("");
console.log(`Scanned ${filesScanned.length} .vue files`);
if (totalFindings === 0) {
  console.log("✅ all write buttons under v-perms / hasPerms");
  process.exit(0);
} else {
  console.log(`❌ ${totalFindings} write buttons without perm guard`);
  console.log("    fix: 加 v-perms=\"['CODE']\" 或 v-if=\"hasPerms(['CODE'])\"");
  process.exit(1);
}