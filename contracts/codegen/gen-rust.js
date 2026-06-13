#!/usr/bin/env node
/**
 * 从 error-codes.yaml 生成 Rust 常量模块
 * 输出：libs/common-error/src/codes.rs
 */

const fs = require('fs');
const path = require('path');

// 简易 YAML 解析（仅支持本文件格式）
function parseYaml(content) {
  const lines = content.split('\n');
  const result = { codes: {} };
  let currentSection = null;
  let currentCode = null;

  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith('#') || trimmed === '') continue;

    if (trimmed.startsWith('version:')) {
      result.version = trimmed.split(':')[1].trim().replace(/"/g, '');
    } else if (trimmed === 'codes:') {
      currentSection = 'codes';
    } else if (currentSection === 'codes' && /^[A-Z_0-9]+:$/.test(trimmed)) {
      currentCode = trimmed.replace(':', '');
      result.codes[currentCode] = {};
    } else if (currentCode && trimmed.startsWith('message:')) {
      result.codes[currentCode].message = trimmed.split(':')[1].trim().replace(/"/g, '');
    } else if (currentCode && trimmed.startsWith('kind:')) {
      result.codes[currentCode].kind = trimmed.split(':')[1].trim();
    }
  }
  return result;
}

function generateRust(data) {
  const codes = Object.entries(data.codes);
  const lines = [
    '// Auto-generated from contracts/error-codes.yaml',
    '// DO NOT EDIT MANUALLY',
    '',
    '/// Error code constants',
    'pub mod codes {',
  ];

  for (const [code, info] of codes) {
    lines.push(`    /// ${info.message}`);
    lines.push(`    pub const ${code}: &str = "${code}";`);
  }

  lines.push('}');
  lines.push('');

  // 生成 message 映射函数
  lines.push('/// Get message for error code');
  lines.push('pub fn message(code: &str) -> &str {');
  lines.push('    match code {');
  for (const [code, info] of codes) {
    lines.push(`        "${code}" => "${info.message}",`);
  }
  lines.push('        _ => "unknown error",');
  lines.push('    }');
  lines.push('}');
  lines.push('');

  // 生成 kind 映射函数
  lines.push('use crate::ErrorKind;');
  lines.push('');
  lines.push('/// Get ErrorKind for error code');
  lines.push('pub fn kind(code: &str) -> ErrorKind {');
  lines.push('    match code {');
  for (const [code, info] of codes) {
    lines.push(`        "${code}" => ErrorKind::${info.kind},`);
  }
  lines.push('        _ => ErrorKind::Internal,');
  lines.push('    }');
  lines.push('}');

  return lines.join('\n');
}

function main() {
  const yamlPath = path.resolve(__dirname, '../error-codes.yaml');
  const outputPath = path.resolve(__dirname, '../../libs/common-error/src/codes.rs');

  const yamlContent = fs.readFileSync(yamlPath, 'utf8');
  const data = parseYaml(yamlContent);
  const rustCode = generateRust(data);

  fs.writeFileSync(outputPath, rustCode);
  console.log(`Generated ${outputPath}`);
}

main();