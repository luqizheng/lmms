#!/usr/bin/env node
/**
 * 契约规范检查工具
 * 验证 OpenAPI 文档、错误码、权限码的规范性
 */

const fs = require('fs');
const path = require('path');
const yaml = require('js-yaml');

const ERRORS = {
  INVALID_FORMAT: 'INVALID_FORMAT',
  MISSING_FIELD: 'MISSING_FIELD',
  DUPLICATE_ENTRY: 'DUPLICATE_ENTRY',
  INVALID_PATTERN: 'INVALID_PATTERN',
  MISSING_FILE: 'MISSING_FILE'
};

function validatePermissionsYaml(filePath) {
  const errors = [];
  
  if (!fs.existsSync(filePath)) {
    errors.push({ file: filePath, error: ERRORS.MISSING_FILE, message: '文件不存在' });
    return errors;
  }
  
  const content = fs.readFileSync(filePath, 'utf8');
  const lines = content.split('\n');
  
  const permissionCodes = new Set();
  const pattern = /^([A-Z_]+):/;
  
  lines.forEach((line, index) => {
    const match = line.match(pattern);
    if (match) {
      const code = match[1];
      
      if (!/^[A-Z]+_[A-Z_]+$/.test(code)) {
        errors.push({
          file: filePath,
          line: index + 1,
          error: ERRORS.INVALID_PATTERN,
          message: `权限码格式不正确: ${code}，应为 SERVICE_RESOURCE_ACTION 格式`
        });
      }
      
      if (permissionCodes.has(code)) {
        errors.push({
          file: filePath,
          line: index + 1,
          error: ERRORS.DUPLICATE_ENTRY,
          message: `重复的权限码: ${code}`
        });
      } else {
        permissionCodes.add(code);
      }
    }
  });
  
  return errors;
}

function validateErrorCodesYaml(filePath) {
  const errors = [];
  
  if (!fs.existsSync(filePath)) {
    errors.push({ file: filePath, error: ERRORS.MISSING_FILE, message: '文件不存在' });
    return errors;
  }
  
  const content = fs.readFileSync(filePath, 'utf8');
  const lines = content.split('\n');
  
  const errorCodes = new Set();
  const pattern = /^([A-Z_]+[0-9]+):/;
  
  lines.forEach((line, index) => {
    const match = line.match(pattern);
    if (match) {
      const code = match[1];
      
      if (!/^[A-Z]+_[A-Z_]+_[0-9]+$/.test(code)) {
        errors.push({
          file: filePath,
          line: index + 1,
          error: ERRORS.INVALID_PATTERN,
          message: `错误码格式不正确: ${code}，应为 SERVICE_CATEGORY_NUMBER 格式`
        });
      }
      
      if (errorCodes.has(code)) {
        errors.push({
          file: filePath,
          line: index + 1,
          error: ERRORS.DUPLICATE_ENTRY,
          message: `重复的错误码: ${code}`
        });
      } else {
        errorCodes.add(code);
      }
    }
  });
  
  return errors;
}

function validateOpenApiSpec(filePath) {
  const errors = [];
  
  if (!fs.existsSync(filePath)) {
    errors.push({ file: filePath, error: ERRORS.MISSING_FILE, message: '文件不存在' });
    return errors;
  }
  
  try {
    const content = fs.readFileSync(filePath, 'utf8');
    const trimmed = content.trim();
    const spec = trimmed.startsWith('{') ? JSON.parse(content) : yaml.load(content);
    
    if (!spec.openapi) {
      errors.push({ file: filePath, error: ERRORS.MISSING_FIELD, message: '缺少 openapi 字段' });
    }
    
    if (!spec.info || !spec.info.title || !spec.info.version) {
      errors.push({ file: filePath, error: ERRORS.MISSING_FIELD, message: '缺少 info.title 或 info.version' });
    }
    
    if (!spec.paths) {
      errors.push({ file: filePath, error: ERRORS.MISSING_FIELD, message: '缺少 paths 字段' });
    } else {
      for (const [pathName, methods] of Object.entries(spec.paths)) {
        for (const [method, operation] of Object.entries(methods)) {
          if (typeof operation === 'object') {
            if (!operation.operationId) {
              errors.push({
                file: filePath,
                error: ERRORS.MISSING_FIELD,
                message: `路径 ${pathName} ${method} 缺少 operationId`
              });
            }
            if (!operation.summary) {
              console.warn(`  ⚠️  路径 ${pathName} ${method} 缺少 summary（建议添加）`);
            }
          }
        }
      }
    }
    
  } catch (e) {
    errors.push({ file: filePath, error: ERRORS.INVALID_FORMAT, message: `JSON 解析错误: ${e.message}` });
  }
  
  return errors;
}

function generateReport(errors) {
  console.log('=== 契约规范检查报告 ===\n');
  
  if (errors.length === 0) {
    console.log('✅ 所有契约规范检查通过');
    return;
  }
  
  const errorsByFile = errors.reduce((acc, err) => {
    if (!acc[err.file]) acc[err.file] = [];
    acc[err.file].push(err);
    return acc;
  }, {});
  
  for (const [file, fileErrors] of Object.entries(errorsByFile)) {
    console.log(`\n## ${file}`);
    fileErrors.forEach(err => {
      const lineInfo = err.line ? ` L${err.line}` : '';
      console.log(`  ⚠️${lineInfo} - ${err.message}`);
    });
  }
  
  console.log(`\n总发现 ${errors.length} 个问题`);
}

function main() {
  const contractsDir = path.join(__dirname, '..', 'contracts');
  const servicesDir = path.join(__dirname, '..', 'services');
  
  const errors = [];
  
  console.log('🔍 检查权限码规范...');
  errors.push(...validatePermissionsYaml(path.join(contractsDir, 'permissions.yaml')));
  
  console.log('🔍 检查错误码规范...');
  errors.push(...validateErrorCodesYaml(path.join(contractsDir, 'error-codes.yaml')));
  
  console.log('🔍 检查 OpenAPI 规范...');
  const openapiFiles = [];
  // 扫描 services/ 目录（Rust 服务）
  fs.readdirSync(servicesDir, { withFileTypes: true })
    .filter(dirent => dirent.isDirectory())
    .forEach(service => {
      const openapiPath = path.join(servicesDir, service.name, 'openapi.yaml');
      if (fs.existsSync(openapiPath)) {
        openapiFiles.push(openapiPath);
        errors.push(...validateOpenApiSpec(openapiPath));
      }
    });
  // 扫描 apps/ 目录（Java 等服务）
  const appsDir = path.join(__dirname, '..', 'apps');
  if (fs.existsSync(appsDir)) {
    fs.readdirSync(appsDir, { withFileTypes: true })
      .filter(dirent => dirent.isDirectory())
      .forEach(app => {
        const openapiPath = path.join(appsDir, app.name, 'openapi.yaml');
        if (fs.existsSync(openapiPath)) {
          openapiFiles.push(openapiPath);
          errors.push(...validateOpenApiSpec(openapiPath));
        }
      });
  }
  
  generateReport(errors);
  
  if (errors.length > 0) {
    process.exit(1);
  }
}

main();