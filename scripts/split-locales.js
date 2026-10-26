#!/usr/bin/env node
/**
 * 把扁平的 locales/<locale>.yaml 按顶层命名空间拆成 locales/<locale>/<ns>.yaml。
 *
 * 为什么要拆：业务模块逐个迁移时，每个模块都要往 i18n 里加自己的文案。
 * 单文件会让并行开发互相冲突；按命名空间分文件后，每个模块独占自己的文件。
 *
 * 用法：node scripts/split-locales.js <locales 目录>
 */
const fs = require("node:fs");
const path = require("node:path");

const localesDir = process.argv[2] || path.resolve(__dirname, "../locales");

function splitNamespace(text) {
  const lines = text.split("\n");
  const chunks = [];
  let current = null;

  for (const line of lines) {
    const isTopLevel = /^[A-Za-z][A-Za-z0-9_-]*:/.test(line);
    if (isTopLevel) {
      current = { name: line.split(":")[0], lines: [line] };
      chunks.push(current);
    } else if (current) {
      current.lines.push(line);
    }
  }

  return chunks;
}

for (const file of fs.readdirSync(localesDir)) {
  const match = file.match(/^([A-Za-z-]+)\.ya?ml$/);
  if (!match) continue;

  const locale = match[1];
  const text = fs.readFileSync(path.join(localesDir, file), "utf8");
  const chunks = splitNamespace(text);

  const localeDir = path.join(localesDir, locale);
  fs.mkdirSync(localeDir, { recursive: true });

  for (const chunk of chunks) {
    const out = path.join(localeDir, `${chunk.name}.yaml`);
    fs.writeFileSync(out, chunk.lines.join("\n").trimEnd() + "\n", "utf8");
    console.log(`  ${file} → ${locale}/${chunk.name}.yaml`);
  }

  fs.rmSync(path.join(localesDir, file));
}

console.log("[split-locales] 完成");
