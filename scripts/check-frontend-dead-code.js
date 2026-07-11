#!/usr/bin/env node
//
// 静态校验 frontend/src/api/：顶层 *.ts 和子目录的 index.ts 必须被 src/** 引用，
// 否则视为 dead pure-admin mock 残留，CI 失败。
//
// 背景：vue-pure-admin 模板带 src/api/{system,list,permission,mock}.ts 等老 mock
// 客户端（调 /user /get-card-list /get-map-info 等后端不存在的端点），以及 mock/ 目录
// 整套测试桩。frontend refactor 接入真后端时容易遗留。删后用此脚本守护，防
// 新建 dead api 文件或 mock/ 目录复活。
//
// 豁免：
//   - 子目录内部 .client.ts / .types.ts 等二级模块（由子目录 index.ts 引用，不直连 src）
//
// 用法：node scripts/check-frontend-dead-code.js
//
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.resolve(__dirname, "..");
const SRC_DIR = path.join(ROOT, "frontend/src");
const API_DIR = path.join(SRC_DIR, "api");

if (!fs.existsSync(API_DIR)) {
  console.error(`x api 目录不存在: ${API_DIR}`);
  process.exit(1);
}

function collectApiFiles() {
  const out = [];
  for (const f of fs.readdirSync(API_DIR)) {
    const full = path.join(API_DIR, f);
    const stat = fs.statSync(full);
    if (stat.isFile() && f.endsWith(".ts")) {
      out.push(full);
    } else if (stat.isDirectory()) {
      const idx = path.join(full, "index.ts");
      if (fs.existsSync(idx)) out.push(idx);
    }
  }
  return out;
}

function apiRelPath(absPath) {
  let rel = path
    .relative(SRC_DIR, absPath)
    .replace(/\.ts$/, "")
    .split(path.sep)
    .join("/");
  // 子目录的 index.ts 由 TS bundler 默认按目录名解析，import 写 `@/api/<dir>` 不是 `@/api/<dir>/index`
  if (rel.endsWith("/index")) {
    rel = rel.slice(0, -"/index".length);
  }
  return rel;
}

function findImporters(apiAbsPath) {
  const rel = apiRelPath(apiAbsPath);
  // 转义正则元字符；匹配 `from "@/<rel>"` 或 `from "@/<rel>.js"`
  const escRel = rel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const re = new RegExp(
    String.raw`from\s+["']@/` + escRel + String.raw`(?:\.js)?["']`
  );

  const hits = [];
  function walk(dir) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      if (
        entry.name === "node_modules" ||
        entry.name === ".git" ||
        entry.name === "dist" ||
        entry.name === "build"
      ) {
        continue;
      }
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        walk(full);
      } else if (/\.(ts|vue|tsx|js|mjs|cjs)$/.test(entry.name)) {
        try {
          const content = fs.readFileSync(full, "utf8");
          if (re.test(content)) hits.push(full);
        } catch (_) {
          // skip unreadable
        }
      }
    }
  }
  walk(SRC_DIR);
  return hits;
}

const files = collectApiFiles();
let ok = true;
const dead = [];

for (const f of files) {
  const rel = path.relative(ROOT, f);
  const importers = findImporters(f);
  if (importers.length === 0) {
    dead.push(rel);
  }
}

if (dead.length > 0) {
  console.error("x frontend dead api 残留（src 无任何引用方）：");
  for (const d of dead) {
    console.error(`    ${d}`);
  }
  console.error("");
  console.error(
    "  修复：要么删文件，要么确认有 import 后再跑（注意 barrel re-export 不会算引用）。"
  );
  ok = false;
}

if (ok) {
  console.log(
    `v frontend dead-code check passed (${files.length} api files, 全部有引用方)`
  );
} else {
  process.exit(1);
}
