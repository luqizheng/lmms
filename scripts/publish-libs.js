#!/usr/bin/env node
// ─────────────────────────────────────────────────────────────────────────────
// scripts/publish-libs.js - 发布 libs/ 共享库 crates 到私有 registry
//
// 按拓扑顺序发布 17 个 crate，临时把内部 path 依赖替换为 version 依赖，
// 发布完自动恢复。是 libs/scripts/publish-libs.sh 的 Node.js 重写版。
//
// 依赖顺序（拓扑）：
//   Layer 0: common-error, common-health, common-jwt, common-ratelimit, common-telemetry
//   Layer 1: common-cache, common-config, common-crypto, common-event,
//            common-http, common-log, common-validator, common-telemetry-axum,
//            jwt-ctx
//   Layer 2: common-db (->error, jwt-ctx), authz (->jwt-ctx, common-jwt),
//            common (->error, db, crypto)
//
// 用法：
//   node scripts/publish-libs.js <VERSION> [--dry-run] [--registry <name>]
//
// 必填位置参数：
//   VERSION            统一版本号（例如 1.0.0）；所有 17 个 crate 应已为该版本
//
// 可选 flag：
//   --dry-run          只验证，不实际发布
//   --registry NAME    发布到指定 registry（默认 kellnr）
//   --help, -h         打印帮助
// ─────────────────────────────────────────────────────────────────────────────

"use strict";

const fs = require("fs");
const path = require("path");
const { execSync, spawnSync } = require("child_process");

// ── 路径 ────────────────────────────────────────────────────────────────────
const ROOT = path.resolve(__dirname, "..");
const LIBS_DIR = path.join(ROOT, "libs");

// ── 发布顺序（拓扑排序）─────────────────────────────────────────────────────
const PUBLISH_ORDER = [
  // Layer 0: 无 lib 依赖
  "common-error",
  "common-health",
  "common-jwt",
  "common-ratelimit",
  "common-telemetry",
  // Layer 1: 仅依赖 Layer 0
  "common-cache",
  "common-config",
  "common-crypto",
  "common-event",
  "common-http",
  "common-log",
  "common-validator",
  "common-telemetry-axum",
  "jwt-ctx",
  // Layer 2: 依赖 Layer 0+1
  "common-db",
  "authz",
  "common",
];

// ── 目录名 -> 包名映射 ────────────────────────────────────────────────────
// Cargo.toml 中 name 字段用下划线（如 digua_common_error），
// 目录名用连字符（如 common-error）。
function dirToPackage(dir) {
  switch (dir) {
    case "authz":
      return "digua_authz";
    case "jwt-ctx":
      return "digua_jwt_ctx";
    case "common":
      return "digua_common";
    default:
      return "digua_" + dir.replace(/-/g, "_");
  }
}

// ── 命令行参数解析 ──────────────────────────────────────────────────────────
function parseArgs() {
  const args = process.argv.slice(2);
  let version = "";
  let dryRun = false;
  let registry = "kellnr";
  let showHelp = false;

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    switch (arg) {
      case "--dry-run":
        dryRun = true;
        break;
      case "--registry":
        if (i + 1 < args.length) {
          registry = args[++i];
        } else {
          console.error("[publish-libs] ERROR: --registry 需要参数");
          showHelp = true;
        }
        break;
      case "--help":
      case "-h":
        showHelp = true;
        break;
      default:
        if (arg.startsWith("-")) {
          console.error(`[publish-libs] 未知参数: ${arg}`);
          showHelp = true;
        } else if (!version) {
          version = arg;
        } else {
          console.error(`[publish-libs] 未知参数: ${arg}`);
          showHelp = true;
        }
        break;
    }
  }

  return { version, dryRun, registry, showHelp };
}

// ── 帮助 ────────────────────────────────────────────────────────────────────
function printHelp() {
  const script = path.basename(__filename);
  console.log(`
用法：
  node scripts/publish-libs.js <VERSION> [--dry-run] [--registry <name>]

必填位置参数：
  VERSION            统一版本号（例如 1.0.0）

可选 flag：
  --dry-run          只验证，不实际发布
  --registry NAME    发布到指定 registry（默认 kellnr）
  --help, -h         打印帮助

示例：
  node scripts/publish-libs.js 1.0.0 --dry-run
  node scripts/publish-libs.js 1.0.0 --registry kellnr
`);
}

// ── 辅助函数 ────────────────────────────────────────────────────────────────
function check(condition, message) {
  if (!condition) {
    console.error(`[publish-libs] ERROR: ${message}`);
    process.exit(1);
  }
}

function warn(message) {
  console.warn(`[publish-libs] WARNING: ${message}`);
}

function info(message) {
  console.log(`[publish-libs] ${message}`);
}

// ── 替换 Cargo.toml 中的 path 依赖为 version + registry ──────────────────
// 处理两种模式：
//   pattern-A:  digua_common_error = { path = "../common-error" }
//   pattern-B:  digua_common_db = { path = "../common-db", features = ["mysql"] }
// 替换后：
//   digua_common_error = { version = "1.0.0", registry = "kellnr" }
//   digua_common_db = { version = "1.0.0", registry = "kellnr", features = ["mysql"] }
function replacePathDeps(tomlPath, version, registry) {
  let content = fs.readFileSync(tomlPath, "utf-8");
  let modified = false;

  for (const dir of PUBLISH_ORDER) {
    const pkgName = dirToPackage(dir);

    // Pattern A: { path = "../dir" }
    const patternA = `${pkgName} = { path = "../${dir}" }`;
    const replacementA = `${pkgName} = { version = "${version}", registry = "${registry}" }`;
    if (content.includes(patternA)) {
      content = content.split(patternA).join(replacementA);
      modified = true;
      info(`  ~ ${pkgName}: path -> version + registry`);
    }

    // Pattern B: { path = "../dir", ... }
    // Match: pkgName = { path = "../dir", (then capture everything after the comma until })
    const patternB = new RegExp(
      `${escapeRegex(pkgName)}\\s*=\\s*\\{\\s*path\\s*=\\s*"\\.\\.\\/${escapeRegex(dir)}"\\s*,\\s*`,
      "g"
    );
    const replacementB = `${pkgName} = { version = "${version}", registry = "${registry}", `;
    if (patternB.test(content)) {
      // Reset regex state
      patternB.lastIndex = 0;
      content = content.replace(patternB, replacementB);
      modified = true;
      info(`  ~ ${pkgName}: path (with features) -> version + registry`);
    }
  }

  return { content, modified };
}

function escapeRegex(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

// ── 执行 cargo publish ─────────────────────────────────────────────────────
function publishCrate(dir, dryRun, registry) {
  const pkgName = dirToPackage(dir);
  const args = [
    "publish",
    "-p", pkgName,
    "--registry", registry,
    "--no-verify",
    "--allow-dirty",
  ];
  if (dryRun) {
    args.push("--dry-run");
  }

  info(`  Publishing ${dir} (pkg: ${pkgName})...`);

  const result = spawnSync("cargo", args, {
    cwd: LIBS_DIR,
    stdio: dryRun ? "inherit" : "pipe",
    encoding: "utf-8",
    maxBuffer: 10 * 1024 * 1024, // 10 MB
  });

  if (result.status !== 0) {
    if (!dryRun) {
      console.error(`[publish-libs]   cargo publish stderr:`);
      if (result.stderr) {
        console.error(result.stderr.split("\n").map(l => `    ${l}`).join("\n"));
      }
      if (result.stdout) {
        console.error(result.stdout.split("\n").map(l => `    ${l}`).join("\n"));
      }
    }
    return false;
  }
  return true;
}

// ── 主流程 ──────────────────────────────────────────────────────────────────
function main() {
  const { version, dryRun, registry, showHelp } = parseArgs();

  if (showHelp) {
    printHelp();
    process.exit(0);
  }

  if (!version) {
    console.error("[publish-libs] ERROR: VERSION 是必填位置参数（例如 1.0.0）");
    console.error("用法: node scripts/publish-libs.js <VERSION> [--dry-run] [--registry <name>]");
    process.exit(2);
  }

  // ── 打印摘要 ────────────────────────────────────────────────────────────
  console.log("============================================");
  console.log(" Libs Publish");
  console.log(` Version:  ${version}`);
  console.log(` Registry: ${registry}`);
  console.log(` Dry-run:  ${dryRun}`);
  console.log("============================================");
  console.log("");

  // ── 前置检查：cargo 是否存在 ──────────────────────────────────────────
  try {
    execSync("cargo --version", { stdio: "pipe" });
  } catch {
    console.error("[publish-libs] ERROR: 'cargo' not found on PATH");
    process.exit(1);
  }

  // ── 前置检查：每个 crate 必须有 description 和 license ─────────────────
  info("-> Checking required fields (description, license)...");
  for (const dir of PUBLISH_ORDER) {
    const tomlPath = path.join(LIBS_DIR, dir, "Cargo.toml");
    if (!fs.existsSync(tomlPath)) {
      console.error(`[publish-libs] ERROR: ${dir}/Cargo.toml not found`);
      process.exit(1);
    }
    const content = fs.readFileSync(tomlPath, "utf-8");
    for (const field of ["description", "license"]) {
      const re = new RegExp(`^${field}\\s*=`, "m");
      if (!re.test(content)) {
        console.error(`[publish-libs] ERROR: ${dir} missing '${field}' in Cargo.toml - required by registry`);
        process.exit(1);
      }
    }
  }
  info("  ✓ All crates have required fields");
  console.log("");

  // ── 版本检查：确认所有 crate 版本与指定版本一致 ───────────────────────
  info(`-> Checking crate versions (expected: ${version})...`);
  let versionMismatch = false;
  for (const dir of PUBLISH_ORDER) {
    const tomlPath = path.join(LIBS_DIR, dir, "Cargo.toml");
    const content = fs.readFileSync(tomlPath, "utf-8");
    const match = content.match(/^version\s*=\s*"([^"]+)"/m);
    if (match) {
      const current = match[1];
      if (current !== version) {
        warn(`  ~ ${dir}: ${current} (expected ${version})`);
        versionMismatch = true;
      } else {
        info(`  ✓ ${dir}: ${version}`);
      }
    } else {
      warn(`  ~ ${dir}: no version field found`);
      versionMismatch = true;
    }
  }
  if (versionMismatch) {
    warn("  Some crates have mismatched versions. Proceeding anyway...");
  }
  console.log("");

  // ── 按拓扑顺序逐个发布（替换+发布+恢复）──────────────────────────────
  info("-> Publishing crates in topological order (per-crate replace+publish+restore)...");
  console.log("");

  let publishFailed = 0;

  for (const dir of PUBLISH_ORDER) {
    const tomlPath = path.join(LIBS_DIR, dir, "Cargo.toml");
    let modified = false;
    let originalContent = "";

    // 备份并替换内部 path 依赖
    // 先检查是否有内部 path 依赖
    const tomlContent = fs.readFileSync(tomlPath, "utf-8");
    let hasInternalDep = false;
    for (const internal of PUBLISH_ORDER) {
      if (internal === dir) continue;
      const pkgName = dirToPackage(internal);
      if (tomlContent.includes(`${pkgName} = { path = "../${internal}"`)) {
        hasInternalDep = true;
        break;
      }
    }

    if (hasInternalDep) {
      originalContent = tomlContent;
      const { content: newContent, modified: m } = replacePathDeps(tomlPath, version, registry);
      if (m) {
        fs.writeFileSync(tomlPath, newContent, "utf-8");
        modified = true;
      }
    }

    // 发布当前 crate
    const success = publishCrate(dir, dryRun, registry);

    if (!success) {
      console.error(`[publish-libs]   ✗ ${dir} publish failed`);
      publishFailed++;
    } else {
      info(`  ✓ ${dir} ${dryRun ? "(dry-run)" : "published"}`);
    }

    // 恢复 Cargo.toml
    if (modified) {
      fs.writeFileSync(tomlPath, originalContent, "utf-8");
      info(`  ~ restored ${dir}/Cargo.toml`);
    }

    console.log("");
  }

  // ── 结果 ────────────────────────────────────────────────────────────────
  if (dryRun) {
    console.log("============================================");
    console.log(" Dry-run complete. No crates were published.");
    console.log(" Run without --dry-run to publish.");
    console.log("============================================");
    process.exit(0);
  } else if (publishFailed > 0) {
    console.log("============================================");
    console.log(` WARNING: ${publishFailed} crate(s) failed to publish.`);
    console.log(" Re-run to retry failed ones (idempotent).");
    console.log("============================================");
    process.exit(1);
  } else {
    console.log("============================================");
    console.log(" All libs crates published successfully!");
    console.log(` Version: ${version}`);
    console.log(` Registry: ${registry}`);
    console.log("============================================");
    console.log("");
    console.log("Next step: consumers add via Cargo.toml:");
    console.log("  [dependencies]");
    console.log(`  common-error = { version = "${version}", registry = "${registry}" }`);
    console.log(`  # or: CARGO_REGISTRIES_KELLNR_TOKEN=xxx cargo add common-error --registry ${registry}`);
  }
}

main();
