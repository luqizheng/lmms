#!/usr/bin/env node
/**
 * i18n key 存在性门禁。
 *
 * 扫描 frontend/src 下所有 .vue / .ts 文件里的 `t("xxx")` / `$t("xxx")` 调用，
 * 校验 `shell.` / `system.` 前缀的 key 在 zh-CN.yaml 与 en.yaml 中**都存在**。
 *
 * 背景：PRISM 壳不再有 pure-admin 的 `menus.*` 命名空间（菜单文案来自 DB），
 * 原门禁失去对象，改为校验真正会写错的 key 拼写。
 *
 * 用法：node scripts/check-i18n-namespace.js
 * 退出码：0 通过；1 有 key 缺失。
 */
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.resolve(__dirname, "..");
const FRONTEND_SRC = path.join(ROOT, "frontend/src");
const LOCALES_DIR = path.join(ROOT, "frontend/locales");

/** 只校验这些前缀（其他命名空间由各自业务仓维护） */
const CHECKED_PREFIXES = ["shell.", "system."];

/* ── YAML 解析：极简缩进解析，够用即可（避免引入 yaml 依赖） ────────── */

function parseYaml(text) {
  const root = {};
  const stack = [{ indent: -1, node: root }];

  for (const rawLine of text.split("\n")) {
    if (!rawLine.trim() || rawLine.trim().startsWith("#")) continue;
    const indent = rawLine.length - rawLine.trimStart().length;
    const line = rawLine.trim();
    const match = line.match(/^([^:]+):\s*(.*)$/);
    if (!match) continue;

    const [, key, rawValue] = match;
    while (stack.length > 1 && indent <= stack[stack.length - 1].indent) {
      stack.pop();
    }
    const parent = stack[stack.length - 1].node;

    if (rawValue === "") {
      const child = {};
      parent[key] = child;
      stack.push({ indent, node: child });
    } else {
      parent[key] = rawValue.replace(/^["']|["']$/g, "");
    }
  }

  return root;
}

function hasKey(node, key) {
  let current = node;
  for (const part of key.split(".")) {
    if (!current || typeof current !== "object" || !(part in current)) return false;
    current = current[part];
  }
  return current !== undefined;
}

/* ── 加载某个 locale 的全部文案 ──────────────────────────────────────── */

/** 递归列出 locale 目录下的 yaml 文件。 */
function listYamlFiles(dir) {
  const out = [];
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) out.push(...listYamlFiles(full));
    else if (/\.ya?ml$/.test(entry.name)) out.push(full);
  }
  return out;
}

/** 深合并（后加载的覆盖同名叶子）。 */
function deepMerge(target, source) {
  for (const [key, value] of Object.entries(source)) {
    const bothObjects =
      value && typeof value === "object" && !Array.isArray(value) &&
      target[key] && typeof target[key] === "object" && !Array.isArray(target[key]);
    if (bothObjects) deepMerge(target[key], value);
    else target[key] = value;
  }
  return target;
}

/**
 * 按 `frontend/src/shell/i18n.ts` 的语义加载某个 locale 的文案。
 *
 * 文案按命名空间分文件：`locales/<locale>/<namespace>.yaml`（namespace 可能是多级路径），
 * 文件内容若只带命名空间自身的键（shell.yaml 里是 `shell:`）要解包一层。
 */
function loadLocaleMessages(locale) {
  const localeDir = path.join(LOCALES_DIR, locale);
  if (!fs.existsSync(localeDir)) {
    console.error(`✗ 找不到 locale 目录: ${path.relative(ROOT, localeDir)}`);
    process.exit(1);
  }

  const messages = {};
  for (const file of listYamlFiles(localeDir)) {
    const rel = path.relative(localeDir, file).replace(/\\/g, "/");
    const namespace = rel.replace(/\.ya?ml$/, "").split("/").join(".");
    const content = parseYaml(fs.readFileSync(file, "utf8"));

    const keys = Object.keys(content);
    const segments = namespace.split(".");
    const lastSegment = segments[segments.length - 1];
    const unwrapped =
      keys.length === 1 && keys[0] === lastSegment ? content[lastSegment] : content;

    let node = messages;
    for (const segment of segments.slice(0, -1)) {
      node[segment] = node[segment] ?? {};
      node = node[segment];
    }
    node[lastSegment] = deepMerge(node[lastSegment] ?? {}, unwrapped);
  }
  return messages;
}

/* ── 扫描源码里的 t() 调用 ──────────────────────────────────────────── */

function walk(dir, out = []) {
  let entries;
  try {
    entries = fs.readdirSync(dir);
  } catch {
    return out;
  }
  for (const entry of entries) {
    const full = path.join(dir, entry);
    if (fs.statSync(full).isDirectory()) walk(full, out);
    else if (/\.(vue|ts|tsx)$/.test(entry) && !/\.test\.ts$/.test(entry)) out.push(full);
  }
  return out;
}

const CALL_RE = /\b(?:t|\$t)\(\s*["']([^"']+)["']/g;

const usedKeys = new Map();
for (const file of walk(FRONTEND_SRC)) {
  const source = fs.readFileSync(file, "utf8");
  for (const match of source.matchAll(CALL_RE)) {
    const key = match[1];
    if (!CHECKED_PREFIXES.some(prefix => key.startsWith(prefix))) continue;
    const rel = path.relative(ROOT, file);
    if (!usedKeys.has(key)) usedKeys.set(key, new Set());
    usedKeys.get(key).add(rel);
  }
}

if (usedKeys.size === 0) {
  console.log("[check-i18n-namespace] 没有需要校验的 i18n key");
  process.exit(0);
}

const zh = loadLocaleMessages("zh-CN");
const en = loadLocaleMessages("en");

const missingZh = [];
const missingEn = [];

for (const [key, files] of usedKeys) {
  if (!hasKey(zh, key)) missingZh.push(`${key}  （${[...files].join(", ")}）`);
  if (!hasKey(en, key)) missingEn.push(`${key}  （${[...files].join(", ")}）`);
}

console.log(`[check-i18n-namespace] 校验 ${usedKeys.size} 个 key`);

if (missingZh.length || missingEn.length) {
  if (missingZh.length) {
    console.error("\n✗ zh-CN.yaml 缺少：");
    for (const item of missingZh) console.error(`  - ${item}`);
  }
  if (missingEn.length) {
    console.error("\n✗ en.yaml 缺少：");
    for (const item of missingEn) console.error(`  - ${item}`);
  }
  process.exit(1);
}

console.log("✓ 所有引用的 i18n key 在两个语言文件中都存在");
