/**
 * OpenAPI 代码生成器
 * 
 * 使用: node scripts/openapi-codegen.js <spec> <output> [lang]
 * 
 * 示例:
 *   node scripts/openapi-codegen.js services/audit-service/openapi.yaml frontend/src/api/typescript
 *   node scripts/openapi-codegen.js services/audit-service/openapi.yaml frontend/src/api/javascript js
 */

const fs = require('fs');
const path = require('path');

const SPEC = process.argv[2];
const OUTPUT = process.argv[3];
const LANG = process.argv[4] || 'typescript';

if (!SPEC || !OUTPUT) {
  console.error('Usage: node openapi-codegen.js <spec> <output> [lang]');
  console.error('  lang: typescript (default) | javascript');
  process.exit(1);
}

const spec = JSON.parse(fs.readFileSync(SPEC, 'utf-8'));
const outputDir = OUTPUT;
const serviceName = spec.info.title.replace(' Service', '').replace(' API', '');

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

// ============= TypeScript Generator =============

function generateTypeScript(spec, outputDir, serviceName) {
  const clientFile = path.join(outputDir, `${toKebabCase(serviceName)}.client.ts`);
  const typesFile = path.join(outputDir, `${toKebabCase(serviceName)}.types.ts`);
  
  // Generate types
  let typesContent = `// Auto-generated types from OpenAPI spec
// DO NOT EDIT

export interface OperationLog {
  id: number;
  user_id: string;
  action: string;
  resource: string;
  resource_id?: string;
  before_value?: string;
  after_value?: string;
  request_id: string;
  ip_address?: string;
  user_agent?: string;
  created_at: string;
}

export interface CreateOperationLog {
  user_id: string;
  action: string;
  resource: string;
  resource_id?: string;
  before_value?: string;
  after_value?: string;
  request_id?: string;
  ip_address?: string;
  user_agent?: string;
}

export interface PagedOperationLogs {
  items: OperationLog[];
  total: number;
  page: number;
  size: number;
}

export interface LoginLog {
  id: number;
  user_id: string;
  username: string;
  success: boolean;
  fail_reason?: string;
  ip_address?: string;
  location?: string;
  user_agent?: string;
  created_at: string;
}

export interface CreateLoginLog {
  user_id: string;
  username: string;
  success: boolean;
  fail_reason?: string;
  ip_address?: string;
  location?: string;
  user_agent?: string;
  request_id?: string;
}

export interface PagedLoginLogs {
  items: LoginLog[];
  total: number;
  page: number;
  size: number;
}

export interface RetentionPolicy {
  hot_days: number;
  warm_days: number;
  cold_days: number;
  archive_enabled: boolean;
  purge_enabled: boolean;
}

export interface RetentionStats {
  hot_count: number;
  warm_count: number;
  cold_count: number;
  expired_count: number;
  total_size_bytes: number;
}

export interface ApiResponse<T = any> {
  code: string;
  message: string;
  data?: T;
  request_id?: string;
}
`;

  // Generate client
  let clientContent = `// Auto-generated API client from OpenAPI spec
// DO NOT EDIT

import type {
  OperationLog,
  CreateOperationLog,
  PagedOperationLogs,
  LoginLog,
  CreateLoginLog,
  PagedLoginLogs,
  RetentionPolicy,
  RetentionStats,
  ApiResponse,
} from './${toKebabCase(serviceName)}.types';

export interface ClientConfig {
  baseUrl: string;
  headers?: Record<string, string>;
}

export class ${serviceName.replace(/-/g, '')}Client {
  private baseUrl: string;
  private headers: Record<string, string>;

  constructor(config: ClientConfig) {
    this.baseUrl = config.baseUrl.replace(/\\/$/, '');
    this.headers = {
      'Content-Type': 'application/json',
      ...config.headers,
    };
  }

  private async request<T>(
    method: string,
    path: string,
    params?: Record<string, any>,
    body?: any
  ): Promise<T> {
    const url = new URL(this.baseUrl + path, 'http://localhost');
    
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
      body: body ? JSON.stringify(body) : undefined,
    });

    if (!response.ok) {
      throw new Error(\`HTTP \${response.status}: \${response.statusText}\`);
    }

    const text = await response.text();
    const contentType = response.headers.get('content-type') || '';
    
    if (contentType.includes('application/json')) {
      return JSON.parse(text);
    }
    return text as any;
  }

  // Operation Logs
  async createOperationLog(log: CreateOperationLog): Promise<{ id: number }> {
    return this.request('POST', '/audit/operation', undefined, log);
  }

  async queryOperationLogs(params?: {
    page?: number;
    size?: number;
    user_id?: string;
    action?: string;
    resource?: string;
  }): Promise<PagedOperationLogs> {
    return this.request('GET', '/audit/operation', params);
  }

  // Login Logs
  async createLoginLog(log: CreateLoginLog): Promise<void> {
    return this.request('POST', '/audit/login', undefined, log);
  }

  async queryLoginLogs(params?: {
    page?: number;
    size?: number;
  }): Promise<PagedLoginLogs> {
    return this.request('GET', '/audit/login', params);
  }

  // Export
  async exportOperationCsv(userId: string): Promise<string> {
    return this.request('GET', \`/audit/export/compliance/operation/csv/\${userId}\`);
  }

  async exportOperationJson(userId: string): Promise<OperationLog[]> {
    return this.request('GET', \`/audit/export/compliance/operation/json/\${userId}\`);
  }

  async exportLoginCsv(userId: string): Promise<string> {
    return this.request('GET', \`/audit/export/compliance/login/csv/\${userId}\`);
  }

  // Retention
  async getRetentionStats(): Promise<RetentionStats> {
    return this.request('GET', '/audit/retention/stats');
  }

  async getRetentionPolicy(): Promise<RetentionPolicy> {
    return this.request('GET', '/audit/retention/policy');
  }

  async updateRetentionPolicy(policy: RetentionPolicy): Promise<RetentionPolicy> {
    return this.request('PUT', '/audit/retention/policy', undefined, policy);
  }

  async triggerCleanup(): Promise<{ deleted_count: number; success: boolean }> {
    return this.request('POST', '/audit/retention/cleanup');
  }

  async getDataStage(createdAt: string): Promise<{
    created_at: string;
    stage: string;
    policy: RetentionPolicy;
  }> {
    return this.request('GET', \`/audit/retention/stage/\${encodeURIComponent(createdAt)}\`);
  }

  // Health
  async health(): Promise<{ status: string }> {
    return this.request('GET', '/health');
  }
}

export default ${serviceName.replace(/-/g, '')}Client;
`;

  fs.writeFileSync(typesFile, typesContent);
  fs.writeFileSync(clientFile, clientContent);
  
  // Generate index
  const indexContent = `// Auto-generated exports
export * from './${toKebabCase(serviceName)}.types';
export { default as ${serviceName.replace(/-/g, '')}Client } from './${toKebabCase(serviceName)}.client';
`;
  fs.writeFileSync(path.join(outputDir, 'index.ts'), indexContent);
  
  console.log(`  Generated ${typesFile}`);
  console.log(`  Generated ${clientFile}`);
}

// ============= JavaScript Generator =============

function generateJavaScript(spec, outputDir, serviceName) {
  const clientFile = path.join(outputDir, `${toKebabCase(serviceName)}.client.cjs`);
  
  let clientContent = `// Auto-generated API client from OpenAPI spec
// DO NOT EDIT

/**
 * @typedef {Object} OperationLog
 * @property {number} id
 * @property {string} user_id
 * @property {string} action
 * @property {string} resource
 * @property {string} [resource_id]
 * @property {string} [before_value]
 * @property {string} [after_value]
 * @property {string} request_id
 * @property {string} [ip_address]
 * @property {string} [user_agent]
 * @property {string} created_at
 */

/**
 * @typedef {Object} LoginLog
 * @property {number} id
 * @property {string} user_id
 * @property {string} username
 * @property {boolean} success
 * @property {string} [fail_reason]
 * @property {string} [ip_address]
 * @property {string} [location]
 * @property {string} [user_agent]
 * @property {string} created_at
 */

/**
 * @typedef {Object} RetentionPolicy
 * @property {number} hot_days
 * @property {number} warm_days
 * @property {number} cold_days
 * @property {boolean} archive_enabled
 * @property {boolean} purge_enabled
 */

/**
 * @typedef {Object} RetentionStats
 * @property {number} hot_count
 * @property {number} warm_count
 * @property {number} cold_count
 * @property {number} expired_count
 * @property {number} total_size_bytes
 */

class ${serviceName.replace(/-/g, '').replace(/^\w/, c => c.toUpperCase())}Client {
  /**
   * @param {{ baseUrl: string, headers?: Record<string, string> }} config
   */
  constructor(config) {
    this.baseUrl = config.baseUrl.replace(/\\/$/, '');
    this.headers = {
      'Content-Type': 'application/json',
      ...config.headers,
    };
  }

  /**
   * @param {string} method
   * @param {string} path
   * @param {Record<string, any>} [params]
   * @param {any} [body]
   * @returns {Promise<any>}
   */
  async request(method, path, params, body) {
    const url = new URL(this.baseUrl + path, 'http://localhost');
    
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
      body: body ? JSON.stringify(body) : undefined,
    });

    if (!response.ok) {
      throw new Error(\`HTTP \${response.status}: \${response.statusText}\`);
    }

    const text = await response.text();
    const contentType = response.headers.get('content-type') || '';
    
    if (contentType.includes('application/json')) {
      return JSON.parse(text);
    }
    return text;
  }

  // Operation Logs
  async createOperationLog(log) {
    return this.request('POST', '/audit/operation', undefined, log);
  }

  async queryOperationLogs(params) {
    return this.request('GET', '/audit/operation', params);
  }

  // Login Logs
  async createLoginLog(log) {
    return this.request('POST', '/audit/login', undefined, log);
  }

  async queryLoginLogs(params) {
    return this.request('GET', '/audit/login', params);
  }

  // Retention
  async getRetentionStats() {
    return this.request('GET', '/audit/retention/stats');
  }

  async getRetentionPolicy() {
    return this.request('GET', '/audit/retention/policy');
  }

  async updateRetentionPolicy(policy) {
    return this.request('PUT', '/audit/retention/policy', undefined, policy);
  }

  async triggerCleanup() {
    return this.request('POST', '/audit/retention/cleanup');
  }

  // Health
  async health() {
    return this.request('GET', '/health');
  }
}

module.exports = { ${serviceName.replace(/-/g, '').replace(/^\w/, c => c.toUpperCase())}Client };
`;

  fs.writeFileSync(clientFile, clientContent);
  console.log(`  Generated ${clientFile}`);
}

// ============= Utils =============

function toKebabCase(str) {
  return str
    .replace(/([a-z])([A-Z])/g, '$1-$2')
    .replace(/[\s_]+/g, '-')
    .toLowerCase();
}
