#!/usr/bin/env node
/**
 * PRISM 规范副本同步门禁
 *
 * PRISM 的散文版规范（Phase 1R 视觉 / Phase 2 体验 / 暗色版归档）原本只存在于
 * 仓库外的设计侧工作区。本脚本保证仓库内副本 `docs/prism/` 与外部草稿区不漂移。
 *
 * 角色：
 *   docs/prism/*.md        → **权威副本**（唯一可提交、可评审、有 diff 有历史）
 *   外部目录（草稿区）      → 设计侧写作用，改完需回灌
 *
 * 用法：
 *   node scripts/check-prism-spec-sync.js                 # 比对（默认草稿区 D:\Workspace\web-concept\design）
 *   node scripts/check-prism-spec-sync.js --sync          # 回灌：外部 → 仓库（单向，不删仓库侧文件）
 *   node scripts/check-prism-spec-sync.js --mirror-root <dir>   # 指定外部草稿区
 *   node scripts/check-prism-spec-sync.js --repo-root <dir>     # 指定仓库副本目录（自测用）
 *
 * 退出码：0 = 一致 / 外部目录不存在（跳过）；1 = 存在漂移
 *
 * 注意：CI 机器上没有外部草稿区，本脚本会打印说明并以 0 退出——不阻断流水线。
 * 散文与 token 的一致性由 frontend/src/styles/prism/tokens.test.ts 负责
 * （它直接解析 docs/prism/phase1-visual-direction.md 的附录）。
 */
const fs = require("node:fs");
const path = require("node:path");

const REPO_ROOT = path.resolve(__dirname, "..");
const REPO_MIRROR_DIR = path.join(REPO_ROOT, "docs", "prism");
const DEFAULT_MIRROR_ROOT = "D:\\Workspace\\web-concept\\design";

/** 需要纳入版本管理的规范文件（相对文件名） */
const SPEC_FILES = [
  "phase1-visual-direction.md",
  "phase2-ux-spec.md",
  "phase1-visual-dark-v1.archived.md"
];

function argValue(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? undefined : process.argv[index + 1];
}

const mirrorRoot = path.resolve(argValue("--mirror-root") ?? DEFAULT_MIRROR_ROOT);
const repoRoot = path.resolve(argValue("--repo-root") ?? REPO_MIRROR_DIR);
const sync = process.argv.includes("--sync");

/** 比较两边同名文件的内容 */
function compare(file) {
  const repoPath = path.join(repoRoot, file);
  const mirrorPath = path.join(mirrorRoot, file);
  const repoExists = fs.existsSync(repoPath);
  const mirrorExists = fs.existsSync(mirrorPath);

  if (!repoExists && !mirrorExists) return { file, state: "both-missing" };
  if (!repoExists) return { file, state: "repo-missing" };
  if (!mirrorExists) return { file, state: "mirror-missing" };

  const same = fs
    .readFileSync(repoPath)
    .equals(fs.readFileSync(mirrorPath));
  return { file, state: same ? "same" : "differs" };
}

function report(result) {
  const map = {
    same: "一致",
    differs: "内容不同",
    "repo-missing": "仓库副本缺失",
    "mirror-missing": "草稿区缺失",
    "both-missing": "两边都缺失"
  };
  return `${result.file.padEnd(36)} ${map[result.state]}`;
}

function main() {
  if (!fs.existsSync(mirrorRoot)) {
    console.log(
      `[check-prism-spec-sync] 草稿区不存在：${mirrorRoot}\n` +
        "  跳过比对（CI / 其他机器的正常情况）。\n" +
        "  散文与 token 的一致性由 frontend/src/styles/prism/tokens.test.ts 保证。"
    );
    return 0;
  }

  const results = SPEC_FILES.map(compare);

  if (sync) {
    const synced = [];
    for (const result of results) {
      if (result.state === "differs" || result.state === "repo-missing") {
        const from = path.join(mirrorRoot, result.file);
        const to = path.join(repoRoot, result.file);
        fs.mkdirSync(path.dirname(to), { recursive: true });
        fs.copyFileSync(from, to);
        synced.push(result.file);
      }
    }
    console.log("[check-prism-spec-sync] 回灌完成（外部 → 仓库）：");
    if (synced.length === 0) console.log("  无需回灌，仓库副本已是最新");
    else synced.forEach(file => console.log(`  已回灌 ${file}`));
    console.log("  提示：回灌后的改动需要提交，否则下次比对会再次报漂移。");
    return 0;
  }

  console.log(`[check-prism-spec-sync] 仓库副本：${repoRoot}`);
  console.log(`[check-prism-spec-sync] 草稿区：  ${mirrorRoot}`);
  results.forEach(result => console.log("  " + report(result)));

  const drifted = results.filter(
    result => result.state !== "same" && result.state !== "both-missing"
  );
  if (drifted.length === 0) {
    console.log("  → 一致，无漂移");
    return 0;
  }
  console.log(
    `  → 存在漂移（${drifted.length} 个文件）。` +
      "若草稿区是最新，用 --sync 回灌；若仓库是最新，改草稿区或直接提交仓库副本。"
  );
  return 1;
}

if (require.main === module) {
  process.exit(main());
}

module.exports = { SPEC_FILES, compare, DEFAULT_MIRROR_ROOT };
