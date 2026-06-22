#!/usr/bin/env node
/**
 * 从OpenAPI文档自动生成E2E测试用例
 * 输出格式：YAML测试用例文件
 */

const fs = require('fs');
const path = require('path');

function parseOpenAPI(openapiPath) {
  const content = fs.readFileSync(openapiPath, 'utf8');
  return typeof content === 'string' ? JSON.parse(content) : content;
}

function generateTestCase(operationId, method, path, schema) {
  const caseId = operationId.toUpperCase().replace(/-/g, '_');
  
  let body = {};
  if (schema && schema.properties) {
    for (const [key, prop] of Object.entries(schema.properties)) {
      if (prop.type === 'string') {
        body[key] = `test_${key}_${'${rand}'}`;
      } else if (prop.type === 'integer' || prop.type === 'number') {
        body[key] = 1;
      } else if (prop.type === 'boolean') {
        body[key] = true;
      } else if (prop.type === 'array') {
        body[key] = [];
      } else if (prop.type === 'object') {
        body[key] = {};
      }
    }
  }
  
  return {
    caseId,
    desc: schema?.description || operationId,
    method,
    path: `/v1${path}`,
    body: Object.keys(body).length > 0 ? body : undefined,
    expectStatus: [200, 201, 204, 502, 503, 404]
  };
}

function generateYAML(group, cases) {
  const lines = [`group: ${group}`];
  lines.push('cases:');
  
  cases.forEach(testCase => {
    lines.push(`  - caseId: ${testCase.caseId}`);
    lines.push(`    desc: ${testCase.desc}`);
    lines.push(`    method: ${testCase.method}`);
    lines.push(`    path: ${testCase.path}`);
    if (testCase.body) {
      lines.push('    body:');
      for (const [key, value] of Object.entries(testCase.body)) {
        lines.push(`      ${key}: ${value}`);
      }
    }
    lines.push(`    expectStatus: ${JSON.stringify(testCase.expectStatus)}`);
    lines.push('');
  });
  
  return lines.join('\n');
}

function main() {
  const openapiPath = process.argv[2];
  const outputPath = process.argv[3];
  
  if (!openapiPath) {
    console.error('用法: node gen-e2e-from-openapi.js <openapi.yaml> [output.yaml]');
    process.exit(1);
  }
  
  const openapi = parseOpenAPI(openapiPath);
  const group = openapi.info.title.replace(' API', '').toLowerCase().replace(/\s+/g, '-');
  
  const cases = [];
  
  for (const [path, methods] of Object.entries(openapi.paths)) {
    for (const [method, operation] of Object.entries(methods)) {
      if (typeof operation === 'object' && operation.operationId) {
        const requestBody = operation.requestBody?.content?.['application/json']?.schema;
        const testCase = generateTestCase(operation.operationId, method.toUpperCase(), path, requestBody);
        cases.push(testCase);
      }
    }
  }
  
  const yamlContent = generateYAML(group, cases);
  
  if (outputPath) {
    fs.writeFileSync(outputPath, yamlContent);
    console.log(`已生成测试用例: ${outputPath}`);
  } else {
    console.log(yamlContent);
  }
}

main();