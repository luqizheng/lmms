#!/usr/bin/env node
/**
 * PRISM 设计约束门禁
 *
 * 规则 1：组件源码禁止裸色值（hex / rgb / rgba）——颜色只能来自 tokens.css。
 * 规则 2：源码引用的 CSS 变量必须在 tokens.css 中已定义。
 *
 * 用法：
 *   node scripts/check-prism-tokens.js              # 扫描默认范围
 *   node scripts/check-prism-tokens.js --report     # 只报告，不以非零码退出
 *
 * 退出码：0 通过；1 存在违规。
 */
const { readFileSync, readdirSync, statSync } = require("node:fs");
const { join, relative, sep } = require("node:path");

const ROOT = join(__dirname, "..");
const FRONTEND_SRC = join(ROOT, "frontend", "src");
const TOKENS_FILE = join(FRONTEND_SRC, "styles", "prism", "tokens.css");

/**
 * 扫描范围。新增 PRISM 代码目录时在此追加。
 * 旧 pure-admin 前端尚未重写，暂不纳入。
 */
const SCAN_ROOTS = [
  join(FRONTEND_SRC, "components", "ui"),
  join(FRONTEND_SRC, "styles", "prism")
];

/** 允许出现裸色值的文件（token 定义处本身） */
const ALLOW_RAW_COLOR_FILES = new Set([TOKENS_FILE]);

/** 允许引用未在 tokens.css 中定义的变量前缀（如第三方库变量） */
const ALLOWED_VAR_PREFIXES = ["el-", "reka-", "vi-"];

const tokens = readFileSync(TOKENS_FILE, "utf8");
const definedTokens = new Set(
  [...tokens.matchAll(/--([a-z0-9-]+)\s*:/g)].map((m) => m[1])
);

const HEX_RE = /(?<![a-zA-Z0-9])#([0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})(?![a-zA-Z0-9])/g;
const RGB_RE = /\brgba?\(/g;
const VAR_RE = /var\(\s*--([a-z0-9-]+)/g;

const reportOnly = process.argv.includes("--report");

function walk(dir, out = []) {
  let entries;
  try {
    entries = readdirSync(dir);
  } catch {
    return out;
  }
  for (const entry of entries) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      walk(full, out);
    } else if (/\.(vue|css|ts|tsx)$/.test(entry) && !/\.test\.ts$/.test(entry)) {
      out.push(full);
    }
  }
  return out;
}

/** 去掉注释后再做裸色值判定，避免注释里的示例值误报 */
function stripComments(line) {
  return line.replace(/\/\/.*$/, "").replace(/\/\*.*?\*\//g, "");
}

const violations = [];

for (const file of SCAN_ROOTS.flatMap((dir) => walk(dir))) {
  if (ALLOW_RAW_COLOR_FILES.has(file)) continue;
  const rel = relative(ROOT, file).split(sep).join("/");
  const source = readFileSync(file, "utf8");

  source.split("\n").forEach((rawLine, index) => {
    const line = stripComments(rawLine);
    if (!line.trim()) return;

    const hex = line.match(HEX_RE);
    if (hex) {
      violations.push(
        `${rel}:${index + 1} 裸色值 ${hex.join(", ")} —— 请改用 tokens.css 变量`
      );
    }

    const rgb = line.match(RGB_RE);
    if (rgb) {
      violations.push(
        `${rel}:${index + 1} 裸色值 ${rgb.join(", ")} —— 请改用 tokens.css 变量`
      );
    }

    for (const m of line.matchAll(VAR_RE)) {
      const name = m[1];
      if (definedTokens.has(name)) continue;
      if (ALLOWED_VAR_PREFIXES.some((p) => name.startsWith(p))) continue;
      violations.push(`${rel}:${index + 1} 引用了未定义变量 --${name}`);
    }
  });
}

if (violations.length > 0) {
  console.error(`[check-prism-tokens] 违规 ${violations.length} 条：`);
  for (const v of violations) console.error("  " + v);
  if (!reportOnly) process.exit(1);
} else {
  console.log(`[check-prism-tokens] 通过：tokens 已定义 ${definedTokens.size} 个`);
}
