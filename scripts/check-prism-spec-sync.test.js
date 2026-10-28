/**
 * PRISM 规范副本同步门禁的自测。
 *
 * 与 check-prism-tokens.test.js 同理：校验器是护栏，护栏自身的洞不能静默存在。
 * 覆盖：一致 / 内容漂移 / 副本缺失 / 草稿区缺失 / 草稿区不存在时跳过 / --sync 回灌。
 */
const { test } = require("node:test");
const assert = require("node:assert");
const { execFileSync } = require("node:child_process");
const { mkdtempSync, writeFileSync, mkdirSync, rmSync, readFileSync } = require("node:fs");
const { tmpdir } = require("node:os");
const { join } = require("node:path");

const SCRIPT = join(__dirname, "check-prism-spec-sync.js");

/** 建一个临时双目录（仓库副本 / 草稿区），返回运行结果 */
function run({ repo = {}, mirror = {}, args = [], mirrorRoot } = {}) {
  const base = mkdtempSync(join(tmpdir(), "prism-spec-"));
  const repoDir = join(base, "repo");
  const mirrorDir = mirrorRoot ?? join(base, "mirror");
  mkdirSync(repoDir, { recursive: true });
  if (!mirrorRoot) mkdirSync(mirrorDir, { recursive: true });
  try {
    for (const [name, content] of Object.entries(repo)) {
      writeFileSync(join(repoDir, name), content, "utf8");
    }
    for (const [name, content] of Object.entries(mirror)) {
      writeFileSync(join(mirrorDir, name), content, "utf8");
    }
    try {
      const stdout = execFileSync(
        "node",
        [
          SCRIPT,
          "--repo-root",
          repoDir,
          "--mirror-root",
          mirrorDir,
          ...args
        ],
        { encoding: "utf8" }
      );
      return { code: 0, output: stdout };
    } catch (error) {
      return {
        code: error.status,
        output: `${error.stdout ?? ""}${error.stderr ?? ""}`
      };
    }
  } finally {
    rmSync(base, { recursive: true, force: true });
  }
}

const A = "phase1-visual-direction.md";
const B = "phase2-ux-spec.md";

test("仓库副本与草稿区一致时通过", () => {
  const result = run({
    repo: { [A]: "a", [B]: "b" },
    mirror: { [A]: "a", [B]: "b" }
  });
  assert.strictEqual(result.code, 0);
  assert.match(result.output, /一致，无漂移/);
});

test("内容漂移时报错并指出差异", () => {
  const result = run({
    repo: { [A]: "a" },
    mirror: { [A]: "changed" }
  });
  assert.strictEqual(result.code, 1);
  assert.match(result.output, /内容不同/);
  assert.match(result.output, /存在漂移/);
});

test("仓库副本缺失时被报出", () => {
  const result = run({
    repo: {},
    mirror: { [A]: "a" }
  });
  assert.strictEqual(result.code, 1);
  assert.match(result.output, /仓库副本缺失/);
});

test("草稿区缺失文件时被报出", () => {
  const result = run({
    repo: { [A]: "a" },
    mirror: {}
  });
  assert.strictEqual(result.code, 1);
  assert.match(result.output, /草稿区缺失/);
});

test("草稿区整体不存在时跳过且退出码为 0（CI 场景）", () => {
  const result = run({
    repo: { [A]: "a" },
    mirror: {},
    mirrorRoot: join(tmpdir(), "prism-spec-not-exist-xyz")
  });
  assert.strictEqual(result.code, 0);
  assert.match(result.output, /草稿区不存在/);
  assert.match(result.output, /跳过比对/);
});

test("--sync 把草稿区内容回灌到仓库副本", () => {
  const base = mkdtempSync(join(tmpdir(), "prism-spec-"));
  const repoDir = join(base, "repo");
  const mirrorDir = join(base, "mirror");
  mkdirSync(repoDir, { recursive: true });
  mkdirSync(mirrorDir, { recursive: true });
  try {
    writeFileSync(join(repoDir, A), "old", "utf8");
    writeFileSync(join(mirrorDir, A), "new", "utf8");
    const stdout = execFileSync(
      "node",
      [
        SCRIPT,
        "--repo-root",
        repoDir,
        "--mirror-root",
        mirrorDir,
        "--sync"
      ],
      { encoding: "utf8" }
    );
    assert.match(stdout, /回灌完成/);
    assert.strictEqual(readFileSync(join(repoDir, A), "utf8"), "new");
  } finally {
    rmSync(base, { recursive: true, force: true });
  }
});
