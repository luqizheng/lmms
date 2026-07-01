#!/usr/bin/env node
/**
 * 静态校验 router/modules/*.ts：每个模块文件至少声明一条 path 路由。
 *
 * 配套 frontend 服务规范：新增视图必须新增 frontend/src/router/modules/<svc>.ts。
 *   此脚本防止「建了 view 但忘了建 module 文件」导致的菜单不出现。
 *
 * 注意：sidebar 数据来自 prue_backend /get-async-routes（prue_menu_configs 表），
 *   模块文件本身不直接产生 sidebar 项。模块文件是路由注册，sidebar 项是
 *   菜单配置项，需通过菜单配置 UI 创建并 publish。两者一一对应，路径前缀要一致。
 */
const fs = require("fs");
const path = require("path");

const MODULES_DIR = path.resolve(__dirname, "../frontend/src/router/modules");

if (!fs.existsSync(MODULES_DIR)) {
  console.error(`✗ modules 目录不存在: ${MODULES_DIR}`);
  process.exit(1);
}

const files = fs
  .readdirSync(MODULES_DIR)
  .filter((f) => f.endsWith(".ts") && f !== "remaining.ts");

let ok = true;
const topLevelPaths = new Set();

for (const f of files) {
  const fullPath = path.join(MODULES_DIR, f);
  const content = fs.readFileSync(fullPath, "utf8");
  const paths = [...content.matchAll(/path:\s*["']([^"']+)["']/g)].map((m) => m[1]);

  if (paths.length === 0) {
    console.error(`✗ ${f}: no path declared`);
    ok = false;
    continue;
  }

  // 顶层路径（前缀段不重复跨文件）
  const tops = paths.map((p) => "/" + p.split("/").filter(Boolean)[0]);
  for (const t of tops) {
    if (topLevelPaths.has(t)) {
      // 同前缀多文件通常是父子模块，正常（如 system.ts + 子项）
      // 不报错；冲突需 review 时人工判断
    } else {
      topLevelPaths.add(t);
    }
  }
  console.log(`  ${f}: ${paths.length} routes`);
}

if (ok) {
  console.log(`\n✅ ${files.length} router module files ok`);
  console.log(`   top-level prefixes: ${[...topLevelPaths].sort().join(", ")}`);
}

process.exit(ok ? 0 : 1);
