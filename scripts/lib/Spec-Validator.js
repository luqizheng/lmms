// Spec-Validator.js
// 校验 tests/specs/ 下 YAML 文件的必填字段
// 不依赖外部模块；用手写解析识别顶层 key
const fs = require('node:fs');
const path = require('node:path');
const { ROOT, log } = require('./utils.js');

const REQUIRED = ['name', 'service', 'type', 'description', 'steps'];
const ALLOWED_SERVICES = ['auth-service', 'user-service', 'api-gateway', 'token-server'];
const ALLOWED_TYPES = ['unit', 'integration'];
const NAME_PATTERN = /^[a-z][a-z0-9_]*$/;

function getYamlValue(lines, key) {
  const re = new RegExp(`^\\s*${escapeRe(key)}\\s*:\\s*(.+?)\\s*$`);
  for (const line of lines) {
    const m = line.match(re);
    if (m) return m[1].trim().replace(/^["']|["']$/g, '');
  }
  return null;
}

function testKeyPresent(lines, key) {
  const re = new RegExp(`^\\s*${escapeRe(key)}\\s*:`);
  for (const line of lines) {
    if (re.test(line)) return true;
  }
  return false;
}

function escapeRe(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function testSpecFile(p) {
  const errors = [];
  const content = fs.readFileSync(p, 'utf8');
  const lines = content.split(/\r?\n/);

  const name = getYamlValue(lines, 'name');
  const service = getYamlValue(lines, 'service');
  const type = getYamlValue(lines, 'type');
  const description = getYamlValue(lines, 'description');

  for (const k of REQUIRED) {
    if (!testKeyPresent(lines, k)) {
      errors.push(`${p} : missing required field '${k}'`);
    } else if (['name', 'service', 'type', 'description'].includes(k)) {
      const v = getYamlValue(lines, k);
      if (v === null || v === '') {
        errors.push(`${p} : '${k}' must have inline value`);
      }
    }
  }
  if (name && !NAME_PATTERN.test(name)) {
    errors.push(`${p} : name '${name}' must match ${NAME_PATTERN}`);
  }
  if (service && !ALLOWED_SERVICES.includes(service)) {
    errors.push(`${p} : service must be one of ${ALLOWED_SERVICES.join(', ')}`);
  }
  if (type && !ALLOWED_TYPES.includes(type)) {
    errors.push(`${p} : type must be 'unit' or 'integration'`);
  }
  return errors;
}

function walkYaml(dir, out) {
  if (!fs.existsSync(dir)) return;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walkYaml(full, out);
    else if (entry.isFile() && entry.name.endsWith('.yaml')) out.push(full);
  }
}

function invokeSpecValidation() {
  const root = path.join(ROOT, 'tests', 'specs');
  const all = [];
  walkYaml(root, all);
  const allErrors = [];
  for (const f of all) {
    allErrors.push(...testSpecFile(f));
  }
  if (allErrors.length > 0) {
    for (const e of allErrors) log.red(e);
    process.exit(1);
  }
  log.green(`[validate] ${all.length} spec files OK`);
}

module.exports = { invokeSpecValidation, testSpecFile };
