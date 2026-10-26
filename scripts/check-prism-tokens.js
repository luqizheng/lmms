#!/usr/bin/env node
/**
 * PRISM 设计约束门禁
 *
 * 规则 1：组件源码禁止裸色值 —— 颜色只能来自 tokens.css。
 *         覆盖 hex / rgb() / rgba() / hsl() / hwb() / lab() / lch() / oklab() / oklch() / color()
 *         以及 CSS 具名色（white / red / …）。
 * 规则 2：源码引用的 CSS 变量必须在 tokens.css 中已定义。
 *
 * 用法：
 *   node scripts/check-prism-tokens.js              # 扫描默认范围
 *   node scripts/check-prism-tokens.js --report     # 只报告，不以非零码退出
 *
 * 退出码：0 通过；1 存在违规。
 */
const { readFileSync, readdirSync, statSync } = require("node:fs");
const { join, relative, resolve, sep } = require("node:path");

const ROOT = join(__dirname, "..");
const FRONTEND_SRC = join(ROOT, "frontend", "src");
const TOKENS_FILE = join(FRONTEND_SRC, "styles", "prism", "tokens.css");
const BRAND_FILE = join(FRONTEND_SRC, "styles", "prism", "tokens.brand.css");

/**
 * 扫描范围。新增 PRISM 代码目录时在此追加。
 * 旧 pure-admin 前端尚未重写，暂不纳入。
 *
 * `--scan-root <dir>` 可覆盖扫描根，仅供本脚本的自测使用。
 */
const DEFAULT_SCAN_ROOTS = [
  join(FRONTEND_SRC, "components", "ui"),
  join(FRONTEND_SRC, "styles", "prism"),
  join(FRONTEND_SRC, "shell")
];

const scanRootIndex = process.argv.indexOf("--scan-root");
const SCAN_ROOTS =
  scanRootIndex >= 0 && process.argv[scanRootIndex + 1]
    ? [resolve(process.argv[scanRootIndex + 1])]
    : DEFAULT_SCAN_ROOTS;

/** 允许出现裸色值的文件（token 定义处本身） */
const ALLOW_RAW_COLOR_FILES = new Set([TOKENS_FILE, BRAND_FILE]);

/** 允许引用未在 tokens.css 中定义的变量前缀（如第三方库变量） */
const ALLOWED_VAR_PREFIXES = ["el-", "reka-", "vi-"];

/** CSS 具名色 —— 出现在颜色类属性上或在值中单独出现时即为裸色值 */
const NAMED_COLORS = [
  "aqua", "aquamarine", "beige", "black", "blue", "brown", "chartreuse", "chocolate",
  "coral", "crimson", "cyan", "darkblue", "darkgreen", "darkred", "fuchsia", "gold",
  "gray", "green", "grey", "indigo", "ivory", "khaki", "lavender", "lime", "linen",
  "magenta", "maroon", "navy", "olive", "orange", "orchid", "pink", "plum", "purple",
  "red", "salmon", "silver", "skyblue", "tan", "teal", "tomato", "turquoise", "violet",
  "wheat", "white", "yellow"
];

/** 允许的 CSS 关键字（非颜色） */
const ALLOWED_KEYWORDS = new Set([
  "transparent",
  "currentcolor",
  "currentColor",
  "inherit",
  "initial",
  "unset",
  "none",
  "auto"
]);

/** 只在这些属性上检查具名色，避免 font-family 等误报 */
const COLOR_PROPS = [
  "color",
  "background",
  "background-color",
  "border",
  "border-color",
  "border-top",
  "border-right",
  "border-bottom",
  "border-left",
  "border-top-color",
  "border-right-color",
  "border-bottom-color",
  "border-left-color",
  "outline",
  "outline-color",
  "fill",
  "stroke",
  "box-shadow",
  "text-shadow",
  "text-decoration-color",
  "caret-color",
  "accent-color",
  "column-rule-color"
];

const tokens = readFileSync(TOKENS_FILE, "utf8");
const definedTokens = new Set(
  [...tokens.matchAll(/--([a-z0-9-]+)\s*:/g)].map((m) => m[1])
);

const HEX_RE = /(?<![a-zA-Z0-9])#([0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})(?![a-zA-Z0-9])/g;
const COLOR_FN_RE = /\b(rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\s*\(/g;
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

/**
 * 逐字符剥离注释，保留换行以维持行号对应。
 * 块注释跨行时，其中的内容整段被忽略（不会被误判为裸色值）。
 */
function stripCommentsKeepingLines(source) {
  let out = "";
  let i = 0;
  const n = source.length;
  while (i < n) {
    const two = source.slice(i, i + 2);
    if (two === "/*") {
      let j = i + 2;
      while (j < n && source.slice(j, j + 2) !== "*/") j++;
      const chunk = source.slice(i, Math.min(j + 2, n));
      out += chunk.replace(/[^\n]/g, " ");
      i = j + 2;
    } else if (two === "//") {
      let j = i;
      while (j < n && source[j] !== "\n") j++;
      out += source.slice(i, j).replace(/[^\n]/g, " ");
      i = j;
    } else {
      out += source[i];
      i++;
    }
  }
  return out;
}

/** 从一行里取出属性名 → 属性值 */
function declarations(line) {
  const pairs = [];
  const re = /([a-zA-Z-]+)\s*:\s*([^;{}]+)/g;
  let m;
  while ((m = re.exec(line)) !== null) {
    pairs.push({ prop: m[1].toLowerCase(), value: m[2].trim() });
  }
  return pairs;
}

const violations = [];

for (const file of SCAN_ROOTS.flatMap((dir) => walk(dir))) {
  if (ALLOW_RAW_COLOR_FILES.has(file)) continue;
  const rel = relative(ROOT, file).split(sep).join("/");
  const source = readFileSync(file, "utf8");
  const cleaned = stripCommentsKeepingLines(source);

  cleaned.split("\n").forEach((line, index) => {
    if (!line.trim()) return;
    const lineNo = index + 1;

    // 规则 2：变量引用完整性
    for (const m of line.matchAll(VAR_RE)) {
      const name = m[1];
      if (definedTokens.has(name)) continue;
      if (ALLOWED_VAR_PREFIXES.some((p) => name.startsWith(p))) continue;
      violations.push(`${rel}:${lineNo} 引用了未定义变量 --${name}`);
    }

    // 规则 1a：整行裸值（模板字符串、类名之外的自由文本）
    const decls = declarations(line);
    const isDeclaration = decls.length > 0 && decls[0].prop !== undefined;

    if (!isDeclaration) {
      const hex = line.match(HEX_RE);
      if (hex) {
        violations.push(
          `${rel}:${lineNo} 裸色值 ${hex.join(", ")} —— 请改用 tokens.css 变量`
        );
      }
      const fn = line.match(COLOR_FN_RE);
      if (fn) {
        violations.push(
          `${rel}:${lineNo} 裸色值 ${fn.join(", ")} —— 请改用 tokens.css 变量`
        );
      }
      return;
    }

    // 规则 1b：声明值里的裸色值
    for (const { prop, value } of decls) {
      const hex = value.match(HEX_RE);
      if (hex) {
        violations.push(
          `${rel}:${lineNo} 裸色值 ${hex.join(", ")} —— 请改用 tokens.css 变量`
        );
      }
      const fn = value.match(COLOR_FN_RE);
      if (fn) {
        violations.push(
          `${rel}:${lineNo} 裸色值 ${fn.join(", ")} —— 请改用 tokens.css 变量`
        );
      }
      // 具名色：值整体是颜色名，或颜色类属性里出现颜色名
      const lowered = value.toLowerCase();
      const wholeValueIsColor =
        NAMED_COLORS.includes(lowered) && !ALLOWED_KEYWORDS.has(lowered);
      const colorProp = COLOR_PROPS.includes(prop);
      const namedInValue = NAMED_COLORS.find(
        (c) => new RegExp(`(^|[\\s,(])${c}($|[\\s,;)])`).test(lowered)
      );
      if (wholeValueIsColor || (colorProp && namedInValue)) {
        violations.push(
          `${rel}:${lineNo} 裸色值 "${namedInValue ?? lowered}" —— 请改用 tokens.css 变量`
        );
      }
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
