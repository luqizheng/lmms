#!/usr/bin/env node
/**
 * 从 error-codes.yaml 和 permissions.yaml 生成 TypeScript 类型
 * 输出：packages/contracts/src/error-codes.ts 和 permissions.ts
 */

const fs = require('fs');
const path = require('path');

function parseYaml(content) {
  // 与 gen-rust.js 相同的解析逻辑
  const lines = content.split('\n');
  const result = { codes: {}, permissions: {} };
  let currentSection = null;
  let currentKey = null;

  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith('#') || trimmed === '') continue;

    if (trimmed.startsWith('version:')) {
      result.version = trimmed.split(':')[1].trim().replace(/"/g, '');
    } else if (trimmed === 'codes:') {
      currentSection = 'codes';
    } else if (trimmed === 'permissions:') {
      currentSection = 'permissions';
    } else if (currentSection && /^[A-Z_0-9]+:$/.test(trimmed)) {
      currentKey = trimmed.replace(':', '');
      result[currentSection][currentKey] = {};
    } else if (currentKey && trimmed.startsWith('message:')) {
      result[currentSection][currentKey].message = trimmed.split(':')[1].trim().replace(/"/g, '');
    } else if (currentKey && trimmed.startsWith('kind:')) {
      result[currentSection][currentKey].kind = trimmed.split(':')[1].trim();
    } else if (currentKey && trimmed.startsWith('description:')) {
      result[currentSection][currentKey].description = trimmed.split(':')[1].trim().replace(/"/g, '');
    }
  }
  return result;
}

function generateTsErrors(data) {
  const codes = Object.keys(data.codes);
  return [
    '// Auto-generated from contracts/error-codes.yaml',
    '// DO NOT EDIT MANUALLY',
    '',
    'export type ErrorCode =',
    ...codes.map(c => `  | '${c}'`),
    ';',
    '',
    'export const ErrorMessages: Record<ErrorCode, string> = {',
    ...Object.entries(data.codes).map(([c, i]) => `  '${c}': '${i.message}',`),
    '};',
  ].join('\n');
}

function generateTsPerms(data) {
  const perms = Object.keys(data.permissions);
  return [
    '// Auto-generated from contracts/permissions.yaml',
    '// DO NOT EDIT MANUALLY',
    '',
    'export type PermissionCode =',
    ...perms.map(p => `  | '${p}'`),
    ';',
    '',
    'export const PermissionDescriptions: Record<PermissionCode, string> = {',
    ...Object.entries(data.permissions).map(([p, i]) => `  '${p}': '${i.description}',`),
    '};',
  ].join('\n');
}

function main() {
  // 输出目标：@digua/contracts 包
  const contractsDir = path.resolve(__dirname, '../../packages/contracts/src');
  if (!fs.existsSync(contractsDir)) {
    console.log('packages/contracts directory not found, skipping TypeScript generation');
    console.log('To enable: create packages/contracts/src/ directory');
    return;
  }

  const errorYaml = fs.readFileSync(path.resolve(__dirname, '../error-codes.yaml'), 'utf8');
  const permYaml = fs.readFileSync(path.resolve(__dirname, '../permissions.yaml'), 'utf8');

  const errorData = parseYaml(errorYaml);
  const permData = parseYaml(permYaml);

  fs.writeFileSync(
    path.join(contractsDir, 'error-codes.ts'),
    generateTsErrors(errorData)
  );
  fs.writeFileSync(
    path.join(contractsDir, 'permissions.ts'),
    generateTsPerms(permData)
  );

  console.log('Generated TypeScript contracts');
}

main();