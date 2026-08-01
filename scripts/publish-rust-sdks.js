#!/usr/bin/env node
// ─────────────────────────────────────────────────────────────────────────────
// scripts/publish-rust-sdks.js - 发布 Rust SDK crates 到私有 registry
//
// 依赖顺序：先发布 digua-http-client（基础库），再发布 6 个 service SDK。
// 各 service SDK 通过 path 依赖 digua-http-client，发布时临时替换为 version
// 依赖，发布完自动恢复。
//
// 前置条件：
//   1. `cargo login --registry <registry>` 已完成
//   2. 网络可访问目标 registry（默认 kellnr，--registry 指定其他）
//   3. 工作区能正常编译（`cargo build -p digua-http-client` 通过）
//
// 用法：
//   node scripts/publish-rust-sdks.js <VERSION> [--dry-run] [--registry <name>]
//
// 必填位置参数：
//   VERSION            统一版本号（例如 0.2.0）；所有 7 个 crate 改到该版本
//
// 可选 flag：
//   --dry-run          只验证，不实际发布
//   --registry NAME    发布到指定 registry（默认 kellnr）
//   --help, -h         打印帮助
// ─────────────────────────────────────────────────────────────────────────────

"use strict";

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");

// ── 默认值 ──────────────────────────────────────────────────────────────────
const ROOT = process.cwd();
const RUST_SDK_DIR = path.join(ROOT, "sdk", "rust");

let crateVersion = "";
let dryRun = false;
let registry = "kellnr";

// ── 发布顺序：基础库在前，service SDK 在后 ──────────────────────────────────
const BASE_CRATES = ["digua-http-client"];

const SERVICE_CRATES = [
  "audit-service-sdk",
  "config-service-sdk",
  "file-service-sdk",
  "iam-service-sdk",
  "notification-service-sdk",
  "token-service-sdk",
];

// Directory name → `cargo publish -p` package name (actual Cargo.toml [package] name)
const PKG_NAMES = {
  "digua-http-client": "digua_http_client",
  "audit-service-sdk": "digua_audit_sdk",
  "config-service-sdk": "digua_config_sdk",
  "file-service-sdk": "digua_file_sdk",
  "iam-service-sdk": "digua_iam_sdk",
  "notification-service-sdk": "digua_notification_sdk",
  "token-service-sdk": "digua_token_sdk",
};

// ── 工具函数 ────────────────────────────────────────────────────────────────

function log(...args) {
  console.log(...args);
}

function logErr(...args) {
  console.error(...args);
}

function yellow(str) {
  return `\x1b[33m${str}\x1b[0m`;
}

function green(str) {
  return `\x1b[32m${str}\x1b[0m`;
}

function red(str) {
  return `\x1b[31m${str}\x1b[0m`;
}

function header(title) {
  console.log("=".repeat(44));
  console.log(` ${title}`);
  console.log("=".repeat(44));
  console.log("");
}

function separator() {
  console.log("");
}

// ── 参数解析 ────────────────────────────────────────────────────────────────

const args = process.argv.slice(2);

if (args.length === 0) {
  logErr("[publish-rust] ERROR: VERSION 是必填位置参数（例如 0.2.0）");
  logErr("用法: node scripts/publish-rust-sdks.js <VERSION> [--dry-run] [--registry <name>]");
  process.exit(2);
}

for (let i = 0; i < args.length; i++) {
  const arg = args[i];
  if (arg === "--help" || arg === "-h") {
    printHelp();
    process.exit(0);
  } else if (arg === "--dry-run") {
    dryRun = true;
  } else if (arg === "--registry") {
    registry = args[i + 1];
    if (!registry) {
      logErr("[publish-rust] ERROR: --registry 需要跟一个名称");
      process.exit(2);
    }
    i++; // 消费 next arg
  } else if (arg.startsWith("-")) {
    logErr(`[publish-rust] 未知参数: ${arg}`);
    printHelp();
    process.exit(2);
  } else {
    if (!crateVersion) {
      crateVersion = arg;
    } else {
      logErr(`[publish-rust] 未知参数: ${arg}`);
      printHelp();
      process.exit(2);
    }
  }
}

if (!crateVersion) {
  logErr("[publish-rust] ERROR: VERSION 是必填位置参数（例如 0.2.0）");
  logErr("用法: node scripts/publish-rust-sdks.js <VERSION> [--dry-run] [--registry <name>]");
  process.exit(2);
}

function printHelp() {
  const script = fs.readFileSync(__filename, "utf-8");
  const lines = script.split("\n");
  // Print comment block: lines starting with // between the shebang and "use strict"
  const start = lines.findIndex((l) => l.startsWith("//"));
  const end = lines.findIndex((l, i) => i > start && !l.startsWith("//") && l.trim() !== "");
  for (let i = start; i < end; i++) {
    let line = lines[i];
    if (line.startsWith("// ")) {
      line = line.slice(3);
    } else if (line === "//") {
      line = "";
    }
    console.log(line);
  }
}

// ── 标题 ────────────────────────────────────────────────────────────────────
log("============================================");
log(" Rust SDK Publish");
log(` Version:  ${crateVersion}`);
log(` Registry: ${registry}`);
log(` Dry-run:  ${dryRun ? "1" : "0"}`);
log("============================================");
separator();

// ── 前置检查 ────────────────────────────────────────────────────────────────

// Check cargo exists
try {
  execSync("cargo --version", { stdio: "ignore" });
} catch {
  logErr("[publish-rust] ERROR: 'cargo' not found on PATH");
  process.exit(1);
}

// Check required fields in every crate
const REQUIRED_FIELDS = ["description", "license"];

function checkRequiredFields(crateDir) {
  const crateName = path.basename(crateDir);
  const tomlPath = path.join(crateDir, "Cargo.toml");
  const content = fs.readFileSync(tomlPath, "utf-8");

  for (const field of REQUIRED_FIELDS) {
    const re = new RegExp(`^${field} `, "m");
    if (!re.test(content)) {
      logErr(
        `[publish-rust] ERROR: ${crateName} missing '${field}' in Cargo.toml — required by crates.io`
      );
      process.exit(1);
    }
  }
}

log("[publish-rust] -> Checking required fields...");
for (const crate of [...BASE_CRATES, ...SERVICE_CRATES]) {
  checkRequiredFields(path.join(RUST_SDK_DIR, crate));
}
log("[publish-rust]   ✓ All crates have required fields");
separator();

// ── 统一版本号 ──────────────────────────────────────────────────────────────
log(`[publish-rust] -> Ensuring all crates have version ${crateVersion}...`);
for (const crate of [...BASE_CRATES, ...SERVICE_CRATES]) {
  const toml = path.join(RUST_SDK_DIR, crate, "Cargo.toml");
  const content = fs.readFileSync(toml, "utf-8");
  const match = content.match(/^version = "(.*)"/m);
  const current = match ? match[1] : "unknown";

  if (current !== crateVersion) {
    if (dryRun) {
      log(`[publish-rust]   ~ ${crate}: ${current} -> ${crateVersion}  (dry-run; not modified)`);
    } else {
      log(`[publish-rust]   ~ ${crate}: ${current} -> ${crateVersion}`);
      const newContent = content.replace(
        /^version = "(.*)"/m,
        `version = "${crateVersion}"`
      );
      fs.writeFileSync(toml, newContent, "utf-8");
    }
  } else {
    log(`[publish-rust]   ✓ ${crate}: ${crateVersion}`);
  }
}
separator();

// ── 发布函数 ────────────────────────────────────────────────────────────────

function publishCrate(crateName) {
  const pkgName = PKG_NAMES[crateName];
  const registryArgs = ["--registry", registry];
  const baseArgs = ["--no-verify", "--allow-dirty"];

  log(`[publish-rust]   Publishing ${crateName}...`);

  if (dryRun) {
    log(
      `[publish-rust]     (dry-run) cargo publish -p ${pkgName} --registry ${registry} --no-verify --allow-dirty`
    );
    try {
      const cmd = ["cargo", "publish", "-p", pkgName, ...registryArgs, ...baseArgs, "--dry-run"].join(" ");
      const output = execSync(cmd, { encoding: "utf-8", stdio: "pipe" });
      output
        .trim()
        .split("\n")
        .forEach((line) => log(`      ${line}`));
    } catch (err) {
      // Dry-run failure is non-fatal
      if (err.stdout) {
        err.stdout
          .trim()
          .split("\n")
          .forEach((line) => log(`      ${line}`));
      }
      if (err.stderr) {
        err.stderr
          .trim()
          .split("\n")
          .forEach((line) => log(`      ${line}`));
      }
    }
  } else {
    try {
      const cmd = ["cargo", "publish", "-p", pkgName, ...registryArgs, ...baseArgs].join(" ");
      execSync(cmd, { encoding: "utf-8", stdio: "inherit" });
    } catch (err) {
      logErr(`[publish-rust]   ✗ ${crateName} publish failed (exit ${err.status})`);
      return false;
    }
  }

  log(`[publish-rust]   ✓ ${crateName} published`);
  return true;
}

// ── Step 1: 发布 digua-http-client（基础库，无 path 依赖问题）─────────────
log(`[publish-rust] -> Step 1/2: Publishing base crate ${BASE_CRATES[0]}...`);
{
  const cwd = process.cwd();
  try {
    process.chdir(path.join(RUST_SDK_DIR, BASE_CRATES[0]));
    publishCrate(BASE_CRATES[0]);
  } finally {
    process.chdir(cwd);
  }
}
separator();

// ── Step 2: 发布 service SDK（需临时替换 path 依赖为 version 依赖）────────
log("[publish-rust] -> Step 2/2: Publishing service SDK crates...");

const backupFiles = [];

if (dryRun) {
  log(
    "[publish-rust]   (dry-run) would replace path deps with version+registry, then publish each crate"
  );
} else {
  for (const crate of SERVICE_CRATES) {
    const toml = path.join(RUST_SDK_DIR, crate, "Cargo.toml");
    const content = fs.readFileSync(toml, "utf-8");

    const pathDepPattern = /digua_http_client = { path = "\.\.\/digua-http-client" }/;
    if (pathDepPattern.test(content)) {
      log(
        `[publish-rust]   ~ ${crate}: replacing path dep with version ${crateVersion} + registry ${registry}`
      );

      // Create backup
      const backup = toml + ".bak";
      fs.copyFileSync(toml, backup);
      backupFiles.push(toml);

      // Replace path dep with version+registry
      const newContent = content.replace(
        pathDepPattern,
        `digua_http_client = { version = "${crateVersion}", registry = "${registry}" }`
      );
      fs.writeFileSync(toml, newContent, "utf-8");
    }
  }
}

// 逐个发布
let publishFailed = 0;
for (const crate of SERVICE_CRATES) {
  const cwd = process.cwd();
  try {
    process.chdir(path.join(RUST_SDK_DIR, crate));
    const ok = publishCrate(crate);
    if (!ok) {
      logErr(`[publish-rust]   ✗ ${crate} failed`);
      publishFailed++;
    }
  } finally {
    process.chdir(cwd);
  }
}

// ── 恢复 path 依赖 ──────────────────────────────────────────────────────────
separator();
log("[publish-rust] -> Restoring path dependencies...");
for (const toml of backupFiles) {
  const backup = toml + ".bak";
  if (fs.existsSync(backup)) {
    fs.copyFileSync(backup, toml);
    fs.unlinkSync(backup);
    log(`[publish-rust]   ✓ restored ${toml}`);
  }
}

// ── 结果 ────────────────────────────────────────────────────────────────────
separator();
if (dryRun) {
  log("============================================");
  log(" Dry-run complete. No crates were published.");
  log(" Run without --dry-run to publish.");
  log("============================================");
} else if (publishFailed > 0) {
  log("============================================");
  log(" WARNING: Some crates failed to publish.");
  log(" Re-run to retry failed ones (idempotent).");
  log("============================================");
  process.exit(1);
} else {
  log("============================================");
  log(" All Rust SDK crates published successfully!");
  log(` Version: ${crateVersion}`);
  log(` Registry: ${registry}`);
  log("============================================");
  separator();
  log("Next step: consumers add via Cargo.toml:");
  log("  [dependencies]");
  log('  iam-service-sdk = "' + crateVersion + '"');
  log("  # or: cargo add iam-service-sdk");
}
