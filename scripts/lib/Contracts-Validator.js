/**
 * 契约校验器
 * 检查生成的代码是否与 YAML 源文件一致
 */

const fs = require('fs');
const path = require('path');

class ContractsValidator {
  constructor() {
    this.contractsDir = path.resolve(__dirname, '../../contracts');
    this.libsDir = path.resolve(__dirname, '../../libs');
  }

  validate() {
    const results = [];

    // 校验错误码
    const errorResult = this.validateErrorCodes();
    results.push(errorResult);

    // 校验权限码
    const permResult = this.validatePermissions();
    results.push(permResult);

    return {
      passed: results.every(r => r.passed),
      results,
    };
  }

  validateErrorCodes() {
    const yamlPath = path.join(this.contractsDir, 'error-codes.yaml');
    const generatedPath = path.join(this.libsDir, 'common-error/src/codes.rs');

    if (!fs.existsSync(yamlPath)) {
      return { name: 'error-codes', passed: false, error: 'YAML source not found' };
    }
    if (!fs.existsSync(generatedPath)) {
      return { name: 'error-codes', passed: false, error: 'Generated file not found' };
    }

    const yamlContent = fs.readFileSync(yamlPath, 'utf8');
    const generatedContent = fs.readFileSync(generatedPath, 'utf8');

    // 检查 YAML 中的每个 code 是否出现在生成的 Rust 文件中
    const yamlCodes = this.extractYamlCodes(yamlContent);
    const missingCodes = yamlCodes.filter(c => !generatedContent.includes(`pub const ${c}`));

    if (missingCodes.length > 0) {
      return {
        name: 'error-codes',
        passed: false,
        error: `Missing codes in generated file: ${missingCodes.join(', ')}`,
      };
    }

    return { name: 'error-codes', passed: true };
  }

  validatePermissions() {
    const yamlPath = path.join(this.contractsDir, 'permissions.yaml');
    const generatedPath = path.join(this.libsDir, 'authz/src/perms.rs');

    if (!fs.existsSync(yamlPath)) {
      return { name: 'permissions', passed: false, error: 'YAML source not found' };
    }
    if (!fs.existsSync(generatedPath)) {
      return { name: 'permissions', passed: false, error: 'Generated file not found' };
    }

    const yamlContent = fs.readFileSync(yamlPath, 'utf8');
    const generatedContent = fs.readFileSync(generatedPath, 'utf8');

    const yamlPerms = this.extractYamlPermissions(yamlContent);
    const missingPerms = yamlPerms.filter(p => !generatedContent.includes(`pub const ${p}`));

    if (missingPerms.length > 0) {
      return {
        name: 'permissions',
        passed: false,
        error: `Missing permissions in generated file: ${missingPerms.join(', ')}`,
      };
    }

    return { name: 'permissions', passed: true };
  }

  extractYamlCodes(content) {
    const codes = [];
    const lines = content.split('\n');
    for (const line of lines) {
      const trimmed = line.trim();
      if (/^[A-Z_0-9]+:$/.test(trimmed) && !trimmed.startsWith('version')) {
        codes.push(trimmed.replace(':', ''));
      }
    }
    return codes;
  }

  extractYamlPermissions(content) {
    const perms = [];
    const lines = content.split('\n');
    for (const line of lines) {
      const trimmed = line.trim();
      if (/^[A-Z_]+:$/.test(trimmed) && !trimmed.startsWith('version')) {
        perms.push(trimmed.replace(':', ''));
      }
    }
    return perms;
  }
}

module.exports = ContractsValidator;