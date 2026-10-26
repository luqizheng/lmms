/**
 * PRISM 设计约束门禁的自测。
 *
 * 校验器本身是"防止 AI 自造数值"的核心护栏，它必须有测试 ——
 * 否则校验器的洞会静默存在，等新的组件带着裸色值进仓库才被发现。
 */
const { test } = require("node:test");
const assert = require("node:assert");
const { execFileSync, spawnSync } = require("node:child_process");
const { mkdtempSync, writeFileSync, mkdirSync, rmSync } = require("node:fs");
const { tmpdir } = require("node:os");
const { join } = require("node:path");

const SCRIPT = join(__dirname, "check-prism-tokens.js");

function run(files) {
  const dir = mkdtempSync(join(tmpdir(), "prism-tokens-"));
  try {
    for (const [name, content] of Object.entries(files)) {
      const full = join(dir, name);
      mkdirSync(join(full, ".."), { recursive: true });
      writeFileSync(full, content, "utf8");
    }
    try {
      const stdout = execFileSync("node", [SCRIPT, "--scan-root", dir], {
        encoding: "utf8"
      });
      return { code: 0, output: stdout };
    } catch (error) {
      return { code: error.status, output: `${error.stdout ?? ""}${error.stderr ?? ""}` };
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("只使用 token 变量的组件通过", () => {
  const result = run({
    "Ok.vue": `<template><div class="x" /></template>
<style scoped>
.x { color: var(--text-primary); background: var(--surface-base); }
</style>
`
  });
  assert.strictEqual(result.code, 0, result.output);
  assert.match(result.output, /通过/);
});

test("裸 hex 被拦截", () => {
  const result = run({ "Bad.vue": `<style scoped>.x { color: #ff0000; }</style>\n` });
  assert.strictEqual(result.code, 1);
  assert.match(result.output, /裸色值 #ff0000/);
});

test("rgb() / hsl() / oklch() 等颜色函数被拦截", () => {
  for (const value of ["rgb(1,2,3)", "rgba(1,2,3,0.5)", "hsl(220,100%,50%)", "oklch(0.6 0.2 260)"]) {
    const result = run({ "Bad.vue": `<style scoped>.x { color: ${value}; }</style>\n` });
    assert.strictEqual(result.code, 1, `未拦截 ${value}`);
    assert.match(result.output, /裸色值/);
  }
});

test("CSS 具名色被拦截", () => {
  const result = run({ "Bad.vue": `<style scoped>.x { color: white; }</style>\n` });
  assert.strictEqual(result.code, 1);
  assert.match(result.output, /裸色值/);
});

test("font-family 中的颜色名词不误报", () => {
  const result = run({
    "Ok.vue": `<style scoped>.x { font-family: "Black", sans-serif; }</style>\n`
  });
  assert.strictEqual(result.code, 0, result.output);
});

test("transparent / currentColor 不误报", () => {
  const result = run({
    "Ok.vue": `<style scoped>
.x { color: transparent; }
.y { border: 1px solid currentColor; }
</style>
`
  });
  assert.strictEqual(result.code, 0, result.output);
});

test("未定义的 CSS 变量被拦截", () => {
  const result = run({ "Bad.vue": `<style scoped>.x { color: var(--not-a-token); }</style>\n` });
  assert.strictEqual(result.code, 1);
  assert.match(result.output, /未定义变量 --not-a-token/);
});

test("跨行块注释里的色值不误报", () => {
  const result = run({
    "Ok.vue": `<style scoped>
/*
 * 品牌色 #2E5BFF 不可随意更改
 * 参见 tokens.css
 */
.x { color: var(--accent); }
</style>
`
  });
  assert.strictEqual(result.code, 0, result.output);
});

test("单行注释里的色值不误报", () => {
  const result = run({
    "Ok.vue": `<style scoped>\n// 品牌色 #2E5BFF\n.x { color: var(--accent); }\n</style>\n`
  });
  assert.strictEqual(result.code, 0, result.output);
});

test("测试文件本身不被扫描", () => {
  const result = run({
    "Bad.test.ts": `const color = "#ff0000";\n`
  });
  assert.strictEqual(result.code, 0, result.output);
});

test("--report 模式只报告不失败", () => {
  const dir = mkdtempSync(join(tmpdir(), "prism-tokens-"));
  try {
    writeFileSync(join(dir, "Bad.vue"), `<style scoped>.x { color: #ff0000; }</style>\n`);
    const result = spawnSync("node", [SCRIPT, "--scan-root", dir, "--report"], {
      encoding: "utf8"
    });
    assert.strictEqual(result.status, 0, "report 模式不应以非零码退出");
    assert.match(`${result.stdout}${result.stderr}`, /裸色值/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
