/**
 * OpenAPI 代码生成器（spec-driven）
 *
 * 使用: node scripts/openapi-codegen.js <spec> <output> [lang]
 *
 * 示例:
 *   node scripts/openapi-codegen.js services/audit-service/openapi.yaml packages/contracts/src/sdk/audit
 *   node scripts/openapi-codegen.js services/audit-service/openapi.yaml packages/contracts/src/sdk/audit js
 *
 * 类型 & client 方法均来自 spec.components.schemas / spec.paths，
 * 不再硬编码业务实体（避免脚本与 spec 漂移）。
 */

const fs = require('fs');
const path = require('path');
const yaml = require('js-yaml');

const SPEC = process.argv[2];
const OUTPUT = process.argv[3];
const LANG = process.argv[4] || 'typescript';

if (!SPEC || !OUTPUT) {
  console.error('Usage: node openapi-codegen.js <spec> <output> [lang]');
  console.error('  lang: typescript (default) | javascript');
  process.exit(1);
}

const raw = fs.readFileSync(SPEC, 'utf-8');
const spec = SPEC.endsWith('.json') ? JSON.parse(raw) : yaml.load(raw);
const outputDir = OUTPUT;
const serviceName = spec.info.title
  .replace(/ Service$/, '')
  .replace(/ API$/, '')
  .replace(/-service$/, '');

console.log(`Generating ${LANG} SDK for ${serviceName}...`);
console.log(`  Spec: ${SPEC}`);
console.log(`  Output: ${outputDir}`);

fs.mkdirSync(outputDir, { recursive: true });

if (LANG === 'typescript' || LANG === 'ts') {
  generateTypeScript(spec, outputDir, serviceName);
} else if (LANG === 'javascript' || LANG === 'js') {
  generateJavaScript(spec, outputDir, serviceName);
} else {
  console.error(`Unsupported language: ${LANG}`);
  process.exit(1);
}

console.log(`\nGenerated SDK at ${outputDir}`);

// ============= Type mapping =============

function refName(ref) {
  return ref.replace(/^#\/components\/schemas\//, '');
}

function isNullable(schema) {
  if (!schema) return false;
  if (schema.nullable === true) return true;
  if (Array.isArray(schema.type)) return schema.type.includes('null');
  return false;
}

function baseType(schema) {
  if (!schema) return 'any';
  if (schema.$ref) return refName(schema.$ref);
  let t = schema.type;
  if (Array.isArray(t)) t = t.find(x => x !== 'null');
  switch (t) {
    case 'string':
      return schema.format === 'binary' ? 'Blob' : 'string';
    case 'integer':
    case 'number':
      return 'number';
    case 'boolean':
      return 'boolean';
    case 'array':
      return `${baseType(schema.items)}[]`;
    case 'object':
      return inlineObject(schema);
    default:
      return 'any';
  }
}

function inlineObject(schema) {
  const props = schema.properties || {};
  const required = new Set(schema.required || []);
  const keys = Object.keys(props).sort();
  const fields = keys.map(k => {
    const opt = required.has(k) ? '' : '?';
    return `${k}${opt}: ${baseType(props[k])}`;
  });
  return `{ ${fields.join('; ')} }`;
}

function tsFieldLine(name, schema, requiredSet) {
  const opt = requiredSet.has(name) || !isNullable(schema) ? '' : '';
  const optMarker = requiredSet.has(name) ? '' : '?';
  return `  ${name}${optMarker}: ${baseType(schema)};`;
}

// ============= TypeScript Generator =============

function generateTypeScript(spec, outputDir, serviceName) {
  const kebab = toKebabCase(serviceName);
  const clientFile = path.join(outputDir, `${kebab}.client.ts`);
  const typesFile = path.join(outputDir, `${kebab}.types.ts`);

  const schemas = spec.components?.schemas || {};
  const schemaNames = Object.keys(schemas).sort();

  // Types
  const typeBlocks = schemaNames.map(name => {
    const s = schemas[name];
    if (s.enum) {
      const vals = s.enum.map(v => JSON.stringify(v)).join(' | ');
      return `export type ${name} = ${vals};`;
    }
    const props = s.properties || {};
    const required = new Set(s.required || []);
    const lines = Object.keys(props).sort().map(k => tsFieldLine(k, props[k], required));
    return `export interface ${name} {\n${lines.join('\n')}\n}`;
  });

  // Generic API response (still useful, but not assumed to be in spec)
  typeBlocks.push(`export interface ApiResponse<T = any> {
  code: string;
  message: string;
  data?: T;
  request_id?: string;
}`);

  const typesContent = `// Auto-generated types from OpenAPI spec
// DO NOT EDIT

${typeBlocks.join('\n\n')}
`;

  // Client methods
  const ops = collectOperations(spec);
  const importedTypes = new Set();
  const methodBlocks = ops.map(op => {
    const { method, urlPath, opId, params, body, returnType } = op;
    params.forEach(p => collectRefs(p.schema, importedTypes));
    if (body) collectRefs(body, importedTypes);
    if (returnType.refs) returnType.refs.forEach(r => importedTypes.add(r));

    const pathParams = params.filter(p => p.in === 'path');
    const queryParams = params.filter(p => p.in === 'query');

    const sig = buildSignature(opId, pathParams, queryParams, body, returnType.ts);
    const urlExpr = buildUrlExpr(urlPath, pathParams);
    const queryArg = queryParams.length ? 'params' : 'undefined';
    const bodyArg = body ? 'body' : 'undefined';

    return `  async ${sig} {
    return this.request("${method.toUpperCase()}", ${urlExpr}, ${queryArg}, ${bodyArg});
  }`;
  });

  const importList = [...importedTypes].sort();
  const importLine = importList.length
    ? `import type {\n${importList.map(t => `  ${t}`).join(',\n')}\n} from "./${kebab}.types";\n\n`
    : '';

  const className = serviceName
    .split(/[-_\s]+/)
    .map(s => s.charAt(0).toUpperCase() + s.slice(1))
    .join('');

  const clientContent = `// Auto-generated API client from OpenAPI spec
// DO NOT EDIT

${importLine}export interface ClientConfig {
  baseUrl: string;
  headers?: Record<string, string>;
}

export class ${className}Client {
  private baseUrl: string;
  private headers: Record<string, string>;

  constructor(config: ClientConfig) {
    this.baseUrl = config.baseUrl.replace(/\\/$/, "");
    this.headers = {
      "Content-Type": "application/json",
      ...config.headers
    };
  }

  private async request<T>(
    method: string,
    path: string,
    params?: Record<string, any>,
    body?: any
  ): Promise<T> {
    const url = new URL(this.baseUrl + path, "http://localhost");

    if (params) {
      Object.entries(params).forEach(([key, value]) => {
        if (value !== undefined && value !== null) {
          url.searchParams.set(key, String(value));
        }
      });
    }

    const response = await fetch(url.toString(), {
      method,
      headers: this.headers,
      body: body ? JSON.stringify(body) : undefined
    });

    if (!response.ok) {
      throw new Error(\`HTTP \${response.status}: \${response.statusText}\`);
    }

    const text = await response.text();
    const contentType = response.headers.get("content-type") || "";

    if (contentType.includes("application/json")) {
      return JSON.parse(text);
    }
    return text as any;
  }

${methodBlocks.join('\n\n')}

  async health(): Promise<{ status: string }> {
    return this.request("GET", "/health");
  }
}

export default ${className}Client;
`;

  fs.writeFileSync(typesFile, typesContent);
  fs.writeFileSync(clientFile, clientContent);

  const indexContent = `// Auto-generated exports
export * from "./${kebab}.types";
export { default as ${className}Client } from "./${kebab}.client";
`;
  fs.writeFileSync(path.join(outputDir, 'index.ts'), indexContent);

  console.log(`  Generated ${typesFile}`);
  console.log(`  Generated ${clientFile}`);
}

// ============= JavaScript Generator =============

function generateJavaScript(spec, outputDir, serviceName) {
  const kebab = toKebabCase(serviceName);
  const clientFile = path.join(outputDir, `${kebab}.client.cjs`);

  const className = serviceName
    .split(/[-_\s]+/)
    .map(s => s.charAt(0).toUpperCase() + s.slice(1))
    .join('');

  const ops = collectOperations(spec);
  const methodBlocks = ops.map(op => {
    const { method, urlPath, opId, params, body } = op;
    const pathParams = params.filter(p => p.in === 'path');
    const queryParams = params.filter(p => p.in === 'query');
    const argList = [
      ...pathParams.map(p => p.name),
      queryParams.length ? `params` : null,
      body ? 'body' : null,
    ].filter(Boolean);
    const urlExpr = buildUrlExpr(urlPath, pathParams);
    const queryArg = queryParams.length ? 'params' : 'undefined';
    const bodyArg = body ? 'body' : 'undefined';
    return `  async ${opId}(${argList.join(', ')}) {
    return this.request("${method.toUpperCase()}", ${urlExpr}, ${queryArg}, ${bodyArg});
  }`;
  });

  const clientContent = `// Auto-generated API client from OpenAPI spec
// DO NOT EDIT

class ${className}Client {
  /**
   * @param {{ baseUrl: string, headers?: Record<string, string> }} config
   */
  constructor(config) {
    this.baseUrl = config.baseUrl.replace(/\\/$/, "");
    this.headers = {
      "Content-Type": "application/json",
      ...config.headers
    };
  }

  async request(method, path, params, body) {
    const url = new URL(this.baseUrl + path, "http://localhost");

    if (params) {
      Object.entries(params).forEach(([key, value]) => {
        if (value !== undefined && value !== null) {
          url.searchParams.set(key, String(value));
        }
      });
    }

    const response = await fetch(url.toString(), {
      method,
      headers: this.headers,
      body: body ? JSON.stringify(body) : undefined
    });

    if (!response.ok) {
      throw new Error(\`HTTP \${response.status}: \${response.statusText}\`);
    }

    const text = await response.text();
    const contentType = response.headers.get("content-type") || "";

    if (contentType.includes("application/json")) {
      return JSON.parse(text);
    }
    return text;
  }

${methodBlocks.join('\n\n')}

  async health() {
    return this.request("GET", "/health");
  }
}

module.exports = { ${className}Client };
`;

  fs.writeFileSync(clientFile, clientContent);
  console.log(`  Generated ${clientFile}`);
}

// ============= Operation extraction =============

function collectOperations(spec) {
  const ops = [];
  const paths = spec.paths || {};
  for (const urlPath of Object.keys(paths).sort()) {
    const item = paths[urlPath];
    for (const method of ['get', 'post', 'put', 'patch', 'delete']) {
      const op = item[method];
      if (!op) continue;
      const opId = op.operationId || `${method}_${urlPath.replace(/[^a-zA-Z0-9]+/g, '_')}`;
      const params = (op.parameters || []).map(p => ({
        name: p.name,
        in: p.in,
        required: !!p.required,
        schema: p.schema || { type: 'string' },
      }));
      const body = extractRequestBody(op.requestBody);
      const returnType = extractResponseType(op.responses);
      ops.push({ method, urlPath, opId: snakeToCamel(opId), params, body, returnType });
    }
  }
  return ops;
}

function extractRequestBody(rb) {
  if (!rb) return null;
  const content = rb.content || {};
  const json = content['application/json'];
  return json?.schema || null;
}

function extractResponseType(responses) {
  if (!responses) return { ts: 'any', refs: new Set() };
  const codes = ['200', '201', '202', '204'];
  const refs = new Set();
  for (const code of codes) {
    const r = responses[code];
    if (!r) continue;
    if (code === '204') return { ts: 'void', refs };
    const content = r.content || {};
    const json = content['application/json'];
    if (json?.schema) {
      collectRefs(json.schema, refs);
      return { ts: baseType(json.schema), refs };
    }
    if (content['text/plain']) return { ts: 'string', refs };
  }
  return { ts: 'any', refs };
}

function collectRefs(schema, set) {
  if (!schema) return;
  if (schema.$ref) {
    set.add(refName(schema.$ref));
    return;
  }
  if (schema.items) collectRefs(schema.items, set);
  if (schema.properties) {
    for (const k of Object.keys(schema.properties)) {
      collectRefs(schema.properties[k], set);
    }
  }
}

function buildSignature(opId, pathParams, queryParams, body, returnTs) {
  const args = [];
  pathParams.forEach(p => args.push(`${p.name}: ${baseType(p.schema)}`));
  if (queryParams.length) {
    const fields = queryParams
      .map(p => `${p.name}?: ${baseType(p.schema)}`)
      .join('; ');
    args.push(`params?: { ${fields} }`);
  }
  if (body) {
    args.push(`body: ${baseType(body)}`);
  }
  return `${opId}(${args.join(', ')}): Promise<${returnTs}>`;
}

function buildUrlExpr(urlPath, pathParams) {
  if (!pathParams.length) return JSON.stringify(urlPath);
  let expr = urlPath;
  pathParams.forEach(p => {
    expr = expr.replace(`{${p.name}}`, `\${encodeURIComponent(${p.name})}`);
  });
  return `\`${expr}\``;
}

// ============= Utils =============

function snakeToCamel(s) {
  return s.replace(/_([a-z0-9])/g, (_, c) => c.toUpperCase());
}

function toKebabCase(str) {
  return str
    .replace(/([a-z])([A-Z])/g, '$1-$2')
    .replace(/[\s_]+/g, '-')
    .toLowerCase();
}
