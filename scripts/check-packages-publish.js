#!/usr/bin/env node
/**
 * @digua/contracts / @digua/http 发布前检查。
 *
 * 用法：node scripts/check-packages-publish.js <next-version>
 * 退出码：0 可发布；1 存在阻断项。
 */
const { execSync } = require("node:child_process");
const { existsSync, readFileSync } = require("node:fs");
const { join } = require("node:path");
const http = require("node:http");

const ROOT = join(__dirname, "..");
const REGISTRY = "http://npm.kudi.repos/";
const PACKAGES = [
  { dir: "frontend/packages/contracts", name: "@digua/contracts" },
  { dir: "frontend/packages/http", name: "@digua/http" }
];

const nextVersion = process.argv[2];
const failures = [];
const notes = [];

function check(label, fn) {
  try {
    const result = fn();
    if (result === false) failures.push(label);
    else notes.push(`OK  ${label}`);
  } catch (error) {
    failures.push(`${label} —— ${error.message}`);
  }
}

/** 私有 registry 探活（用 node http，避免跨平台 curl 参数差异） */
function pingRegistry(url) {
  return new Promise((resolve, reject) => {
    const req = http.get(url, res => {
      res.resume();
      resolve(res.statusCode ?? 0);
    });
    req.on("error", reject);
    req.setTimeout(15000, () => req.destroy(new Error("连接超时")));
  });
}

for (const pkg of PACKAGES) {
  const pkgJsonPath = join(ROOT, pkg.dir, "package.json");
  let pkgJson = {};

  check(`${pkg.name}: package.json 存在且可解析`, () => {
    pkgJson = JSON.parse(readFileSync(pkgJsonPath, "utf8"));
    return true;
  });

  check(`${pkg.name}: 名称正确`, () => pkgJson.name === pkg.name);
  check(`${pkg.name}: 未标记 private`, () => pkgJson.private !== true);
  check(
    `${pkg.name}: publishConfig 指向私有 registry`,
    () => pkgJson.publishConfig?.registry === REGISTRY
  );

  if (nextVersion) {
    check(`${pkg.name}: 当前版本低于目标版本 ${nextVersion}`, () => {
      const current = String(pkgJson.version).split(".").map(Number);
      const next = String(nextVersion).split(".").map(Number);
      if (next.length !== 3 || next.some(Number.isNaN)) {
        throw new Error(`目标版本格式非法：${nextVersion}`);
      }
      for (let i = 0; i < 3; i++) {
        if (next[i] > current[i]) return true;
        if (next[i] < current[i]) return false;
      }
      return false;
    });
  }

  check(`${pkg.name}: dist 已构建`, () => {
    if (!existsSync(join(ROOT, pkg.dir, "dist", "index.js"))) {
      throw new Error("dist/index.js 不存在，先运行 pnpm run packages:build");
    }
    return true;
  });
}

check("工作区无未提交的包内改动", () => {
  const diff = execSync(
    `git status --porcelain -- ${PACKAGES.map(p => p.dir).join(" ")}`,
    { cwd: ROOT, encoding: "utf8" }
  ).trim();
  if (diff) throw new Error(`存在未提交改动：\n${diff}`);
  return true;
});

pingRegistry(REGISTRY)
  .then(code => {
    if (!String(code).startsWith("2") && !String(code).startsWith("3")) {
      failures.push(`私有 registry 可达（${REGISTRY}） —— HTTP ${code}`);
    } else {
      notes.push(`OK  私有 registry 可达（${REGISTRY}） —— HTTP ${code}`);
    }
  })
  .catch(error => {
    failures.push(`私有 registry 可达（${REGISTRY}） —— ${error.message}`);
  })
  .then(() => {
    for (const note of notes) console.log(note);

    if (failures.length > 0) {
      console.error("\n[check-packages-publish] 阻断项：");
      for (const failure of failures) console.error(`  ✗ ${failure}`);
      process.exit(1);
    }

    console.log("[check-packages-publish] 通过，可以发布");
  });
