#!/usr/bin/env node
/**
 * 从 tool-codes.yaml 生成 JS 常量模块
 * 输出：packages/cli/lib/tool-codes.js
 *
 * 与 gen-rust.js 同样的手写 YAML 解析（本仓格式固定，无需引入解析器）。
 */

const fs = require('fs');
const path = require('path');

const REPO_ROOT = path.resolve(__dirname, '../..');
const SRC = path.join(REPO_ROOT, 'contracts', 'tool-codes.yaml');
const OUT = path.join(REPO_ROOT, 'packages', 'cli', 'lib', 'tool-codes.js');

// 简易 YAML 解析（仅支持本文件格式：2 空格缩进的 KEY: 块 + 4 空格缩进的字段）
function parseYaml(content) {
  const lines = content.split('\n');
  const result = { version: '', codes: {} };
  let currentCode = null;

  for (const raw of lines) {
    const line = raw.replace(/\s+#.*$/, '').trimEnd();
    if (!line.trim() || line.trim().startsWith('#')) continue;

    const version = line.match(/^version:\s*(.*)$/);
    if (version) { result.version = version[1].trim().replace(/^"|"$/g, ''); continue; }

    if (/^codes:\s*$/.test(line)) continue;

    const code = line.match(/^  ([A-Z_0-9]+):\s*$/);
    if (code) { currentCode = code[1]; result.codes[currentCode] = {}; continue; }

    const field = line.match(/^    ([a-z]+):\s*(.*)$/);
    if (field && currentCode) {
      result.codes[currentCode][field[1]] = field[2].trim().replace(/^"|"$/g, '');
    }
  }
  return result;
}

function generate(data) {
  const codes = Object.keys(data.codes).sort();
  const lines = [
    '// Auto-generated from contracts/tool-codes.yaml',
    '// DO NOT EDIT MANUALLY',
    '',
    `const TOOL_CODES = {`,
  ];
  for (const code of codes) {
    const def = data.codes[code];
    lines.push(`  ${code}: { message: ${JSON.stringify(def.message)}, kind: ${JSON.stringify(def.kind)}, exit: ${Number(def.exit)} },`);
  }
  lines.push('};');
  lines.push('');
  lines.push('module.exports = { TOOL_CODES };');
  lines.push('');
  return lines.join('\n');
}

const data = parseYaml(fs.readFileSync(SRC, 'utf-8'));
const missing = Object.entries(data.codes)
  .filter(([, def]) => !def.message || !def.kind || def.exit === undefined)
  .map(([code]) => code);
if (missing.length > 0) {
  console.error(`[gen-tool-codes] incomplete code definitions: ${missing.join(', ')}`);
  process.exit(1);
}

fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, generate(data));
console.log(`[gen-tool-codes] ${Object.keys(data.codes).length} codes -> ${path.relative(REPO_ROOT, OUT)}`);
