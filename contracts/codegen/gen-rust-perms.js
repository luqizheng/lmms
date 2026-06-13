#!/usr/bin/env node
/**
 * 从 permissions.yaml 生成 Rust 权限码常量模块
 * 输出：libs/authz/src/perms.rs
 */

const fs = require('fs');
const path = require('path');

function parseYaml(content) {
  const lines = content.split('\n');
  const result = { permissions: {} };
  let currentSection = null;
  let currentPerm = null;

  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith('#') || trimmed === '') continue;

    if (trimmed.startsWith('version:')) {
      result.version = trimmed.split(':')[1].trim().replace(/"/g, '');
    } else if (trimmed === 'permissions:') {
      currentSection = 'permissions';
    } else if (currentSection === 'permissions' && /^[A-Z_]+:$/.test(trimmed)) {
      currentPerm = trimmed.replace(':', '');
      result.permissions[currentPerm] = {};
    } else if (currentPerm && trimmed.startsWith('description:')) {
      result.permissions[currentPerm].description = trimmed.split(':')[1].trim().replace(/"/g, '');
    }
  }
  return result;
}

function generateRust(data) {
  const perms = Object.entries(data.permissions);
  const lines = [
    '// Auto-generated from contracts/permissions.yaml',
    '// DO NOT EDIT MANUALLY',
    '',
    '/// Permission code constants',
    'pub mod perms {',
  ];

  for (const [perm, info] of perms) {
    lines.push(`    /// ${info.description}`);
    lines.push(`    pub const ${perm}: &str = "${perm}";`);
  }

  lines.push('}');
  lines.push('');

  // 生成检查函数
  lines.push('/// Check if permission code is valid');
  lines.push('pub fn is_valid(perm: &str) -> bool {');
  lines.push('    matches!(perm,');
  for (let i = 0; i < perms.length; i++) {
    const [perm, _] = perms[i];
    if (i === perms.length - 1) {
      lines.push(`        "${perm}"`);
    } else {
      lines.push(`        "${perm}"`);
      lines.push('        |');
    }
  }
  lines.push('    )');
  lines.push('}');

  return lines.join('\n');
}

function main() {
  const yamlPath = path.resolve(__dirname, '../permissions.yaml');
  const outputPath = path.resolve(__dirname, '../../libs/authz/src/perms.rs');

  const yamlContent = fs.readFileSync(yamlPath, 'utf8');
  const data = parseYaml(yamlContent);
  const rustCode = generateRust(data);

  fs.writeFileSync(outputPath, rustCode);
  console.log(`Generated ${outputPath}`);
}

main();