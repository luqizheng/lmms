#!/usr/bin/env node
/**
 * check-i18n-namespace.js — i18n namespace 错位门禁
 *
 * 防止 P2-7 已修复的"menus.* 调用方 vs yaml namespace"错位 bug 回归。
 * 扫描 frontend/src 下所有 .vue 和 .ts 文件中的 t('menus.X') 调用，
 * 校验 X 必须定义在 frontend/locales 下 zh-CN.yaml 与 en.yaml 的
 * `menus:` namespace 下。
 *
 * 2026-07-30 首次建立——背景是 P2-7 修复了 10 处 namespace 错位
 * （commit c2d23b4），需要 CI gate 防止后续 commit 再引入。
 *
 * 退出码：
 *   0 — 通过（无错位）
 *   1 — 有硬失败（menus.* 调用但 key 不在 menus: namespace 或 yaml 中缺失）
 *   2 — 配置错误
 *
 * 用法：node scripts/check-i18n-namespace.js
 *      node scripts/check-i18n-namespace.js --json
 */

const fs = require("node:fs");
const path = require("node:path");

const REPO_ROOT = path.resolve(__dirname, "..");
const FRONTEND_SRC = path.join(REPO_ROOT, "frontend/src");
const LOCALES_DIR = path.join(REPO_ROOT, "frontend/locales");
const YAML_FILES = ["zh-CN.yaml", "en.yaml"].map((f) =>
  path.join(LOCALES_DIR, f)
);

const JSON_MODE = process.argv.includes("--json");

// Match t('menus.X') or t("menus.X") or $t('menus.X')
// Also catches vue-i18n-composable's useI18n().t('menus.X')
const CALL_PATTERN = /[t$]\(\s*['"]menus\.([A-Za-z][A-Za-z0-9_]*)['"]/g;

// Top-level namespace: e.g. "menus:", "buttons:", "login:"
const NS_RE = /^([a-zA-Z][a-zA-Z0-9_-]*):\s*$/;
// Key line under namespace: "  keyName: value"
const KEY_RE = /^  ([A-Za-z][A-Za-z0-9_]*):\s*/;

function walk(dir, exts) {
  const out = [];
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...walk(full, exts));
    } else if (entry.isFile() && exts.some((e) => entry.name.endsWith(e))) {
      out.push(full);
    }
  }
  return out;
}

function collectCalledKeys() {
  const called = new Map(); // key → [{ file, line }]
  for (const file of walk(FRONTEND_SRC, [".vue", ".ts"])) {
    const content = fs.readFileSync(file, "utf-8");
    const rel = path.relative(REPO_ROOT, file);
    const lines = content.split("\n");
    lines.forEach((line, idx) => {
      const re = new RegExp(CALL_PATTERN.source, "g");
      let m;
      while ((m = re.exec(line)) !== null) {
        const key = m[1];
        if (!called.has(key)) called.set(key, []);
        called.get(key).push({ file: rel, line: idx + 1 });
      }
    });
  }
  return called;
}

function loadYamlNamespaceMap(yamlPath) {
  // Returns { key: namespace }
  const result = {};
  let curNs = null;
  const content = fs.readFileSync(yamlPath, "utf-8");
  for (const line of content.split("\n")) {
    const stripped = line.replace(/\r$/, "");
    const nsMatch = NS_RE.exec(stripped);
    if (nsMatch) {
      curNs = nsMatch[1];
      continue;
    }
    const keyMatch = KEY_RE.exec(stripped);
    if (keyMatch && curNs) {
      result[keyMatch[1]] = curNs;
    }
  }
  return result;
}

function run() {
  // Validate config
  for (const yf of YAML_FILES) {
    if (!fs.existsSync(yf)) {
      console.error(`FATAL: locale file missing: ${yf}`);
      process.exit(2);
    }
  }

  const called = collectCalledKeys();
  if (called.size === 0) {
    console.error(
      `FATAL: no t('menus.X') calls found under ${path.relative(REPO_ROOT, FRONTEND_SRC)}`
    );
    process.exit(2);
  }

  const namespaces = YAML_FILES.map((p) => ({
    path: p,
    name: path.basename(p),
    map: loadYamlNamespaceMap(p),
  }));

  // Cross-check: for each called key, verify it lives under menus: in BOTH yaml files.
  const issues = [];
  for (const [key, refs] of called.entries()) {
    for (const ns of namespaces) {
      const where = ns.map[key];
      if (where === undefined) {
        issues.push({
          type: "missing-in-yaml",
          key,
          file: path.relative(REPO_ROOT, ns.path),
          sampleRef: refs[0],
        });
      } else if (where !== "menus") {
        issues.push({
          type: "wrong-namespace",
          key,
          file: path.relative(REPO_ROOT, ns.path),
          currentNs: where,
          sampleRef: refs[0],
        });
      }
    }
  }

  // Report
  if (JSON_MODE) {
    console.log(JSON.stringify({ called: called.size, issues }, null, 2));
  } else if (issues.length === 0) {
    console.log(
      `✓ i18n namespace check passed: ${called.size} unique menus.* keys all defined under 'menus:' in zh-CN / en`
    );
    process.exit(0);
  } else {
    console.error(
      `✗ i18n namespace check failed: ${issues.length} issue(s) across ${called.size} called keys\n`
    );
    for (const i of issues) {
      const loc = `${i.sampleRef.file}:${i.sampleRef.line}`;
      if (i.type === "wrong-namespace") {
        console.error(
          `  [wrong-namespace] ${i.key} → defined under '${i.currentNs}:' in ${i.file} (expected 'menus:')\n    first call site: ${loc}`
        );
      } else {
        console.error(
          `  [missing-in-yaml] ${i.key} → not defined in ${i.file}\n    first call site: ${loc}`
        );
      }
    }
    console.error(
      "\nFix: move the key into 'menus:' namespace (or under its actual usage namespace) in both yaml files."
    );
  }

  process.exit(1);
}

run();