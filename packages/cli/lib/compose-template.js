'use strict';

// Publish the infrastructure/ compose layout as a versioned base template.
// Downstream repos (ai-pm) copy the bundle and adapt it; same compose project
// name ("infrastructure" dir basename) auto-joins the superpowers network.
//
// 方言变体（2026-11-03）: `dialect: 'pg'` 产出一个以 PostgreSQL 为默认数据库的
// 模板 —— 目录名加 `-pg` 后缀，与 MySQL 默认模板并存。变换规则：
//   - docker-compose-infra.yml: postgres 摘除 profiles(成为默认), mysql 挂
//     profiles: ["mysql"](降级为可选)；卷两侧都保留
//   - docker-compose-services.yml / docker-compose.yml: 所有 `depends_on: mysql`
//     翻转为 postgres；token-service 的 TOKEN_DATABASE_DB_TYPE → postgres；
//     docker-compose.yml 的 mysql-replica 服务与卷整体移除(PG 无对应物)
//   - docker-compose.prod.yml: 文本追加 postgres 的 ports !override 块
//     （该文件含 !override 自定义 tag，js-yaml 不可解析，走文本变换）
//   - .env / .env.prod.example: DATABASE_URL 换成 postgres:// 连接串
//   - mysql-replica/ 与 mysql-init/ 目录不进 PG 包

const fs = require('node:fs');
const path = require('node:path');
const yaml = require('js-yaml');

// Top-level names never shipped in a template bundle. README.md is lmms-owned
// documentation — consuming repos write their own (ruling 2026-10-30: template
// docs belong to the consuming project).
const EXCLUDE_NAMES = new Set(['AGENTS.md', 'STATE.md', 'LOOP.md', 'README.md', 'vendor']);
// Session/loop scratch notes at any depth.
const EXCLUDE_PATTERNS = [/^loop-.*\.md$/];
// PG 变体不携带的 MySQL 专属目录。
const PG_EXCLUDE_DIRS = new Set(['mysql-replica', 'mysql-init']);

const DIALECTS = new Set(['mysql', 'pg']);

function isExcluded(relSegments, name, dialect) {
  if (EXCLUDE_PATTERNS.some((re) => re.test(name))) return true;
  if (relSegments.length === 0 && EXCLUDE_NAMES.has(name)) return true;
  // dotfiles/dirs are skipped, except the whitelisted env example
  if (name.startsWith('.') && name !== '.env.prod.example') return true;
  if (dialect === 'pg' && relSegments.length > 0 && PG_EXCLUDE_DIRS.has(relSegments[0])) return true;
  return false;
}

// Walk infraDir breadth-first; return [{ rel, abs }] with forward-slash rel.
function collectTemplateFiles(infraDir, dialect = 'mysql', fsMod = fs) {
  const out = [];
  const walk = (dir, segments) => {
    for (const entry of fsMod.readdirSync(dir, { withFileTypes: true })) {
      if (isExcluded(segments, entry.name, dialect)) continue;
      const abs = path.join(dir, entry.name);
      const rel = [...segments, entry.name].join('/');
      if (entry.isDirectory()) {
        walk(abs, [...segments, entry.name]);
      } else if (entry.isFile()) {
        out.push({ rel, abs });
      }
    }
  };
  walk(infraDir, []);
  return out.sort((a, b) => a.rel.localeCompare(b.rel));
}

// ---------------------------------------------------------------------------
// PG 方言变换（按文件；未知文件原样拷贝）
// ---------------------------------------------------------------------------

// 结构化 YAML 变换的目标文件（无自定义 tag，js-yaml 可安全往返）。
const YAML_TRANSFORM_FILES = new Set([
  'docker-compose-infra.yml',
  'docker-compose-services.yml',
  'docker-compose.yml',
]);

/** postgres 成为默认，mysql 降级为 profiles: ["mysql"] 可选项。 */
function transformInfraCompose(doc) {
  const services = doc.services ?? {};
  if (services.postgres) delete services.postgres.profiles;
  if (services.mysql) services.mysql.profiles = ['mysql'];
  return doc;
}

/** depends_on 的 mysql 键翻转为 postgres；token 的 DB_TYPE 指向 postgres。 */
function transformAppCompose(doc) {
  const services = doc.services ?? {};
  for (const svc of Object.values(services)) {
    if (svc && svc.depends_on && Object.prototype.hasOwnProperty.call(svc.depends_on, 'mysql')) {
      const entries = Object.entries(svc.depends_on).map(([k, v]) =>
        k === 'mysql' ? ['postgres', v] : [k, v],
      );
      svc.depends_on = Object.fromEntries(entries);
    }
    if (svc && svc.environment && svc.environment.TOKEN_DATABASE_DB_TYPE === 'mysql') {
      svc.environment.TOKEN_DATABASE_DB_TYPE = 'postgres';
    }
  }
  // dev 全栈里的 mysql-replica（含卷）在 PG 变体中整体移除。
  if (services['mysql-replica']) delete services['mysql-replica'];
  if (doc.volumes && doc.volumes.mysql_replica_data) delete doc.volumes.mysql_replica_data;
  return doc;
}

/** .env / .env.prod.example 的 DATABASE_URL 换成 postgres 连接串。 */
function transformEnvFile(text) {
  const pgUrl =
    'DATABASE_URL="postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@${POSTGRES_HOST}:${POSTGRES_PORT}/${POSTGRES_DB}"';
  if (/^DATABASE_URL="postgres:\/\//m.test(text)) return text;
  if (/^DATABASE_URL=/m.test(text)) {
    return text.replace(/^DATABASE_URL=.*$/m, pgUrl);
  }
  return text.replace(/\n$/, '\n') + pgUrl + '\n';
}

/** prod overlay 是文本态（!override tag）：追加 postgres 的端口收敛块。 */
function transformProdOverlay(text) {
  if (/^  postgres:$/m.test(text)) return text;
  const block =
    '\n# PG 方言变体追加：postgres 生产态不发布端口（与上方 mysql 同款收敛）。\n' +
    '  postgres:\n' +
    '    ports: !override []\n';
  return text.replace(/\n*$/, '\n') + block;
}

/** 对单个模板文件做 PG 方言变换；返回变换后的文本（无需变换返回 null）。 */
function transformForPg(rel, text) {
  if (YAML_TRANSFORM_FILES.has(rel)) {
    const doc = yaml.load(text);
    const transformed =
      rel === 'docker-compose-infra.yml'
        ? transformInfraCompose(doc)
        : transformAppCompose(doc);
    return yaml.dump(transformed, { lineWidth: -1, noRefs: true });
  }
  if (rel === 'docker-compose.prod.yml') return transformProdOverlay(text);
  if (rel === '.env' || rel === '.env.prod.example') return transformEnvFile(text);
  return null;
}

// Build <outRoot>/coder-compose-template-<version>[-pg]/ from infraDir.
// Overwrites a previous bundle of the same version+dialect. Returns { dir, fileCount }.
function buildComposeTemplate({ infraDir, outRoot, version, dialect = 'mysql', fsMod = fs }) {
  if (!DIALECTS.has(dialect)) throw new Error(`invalid dialect: ${dialect} (allowed: mysql | pg)`);
  const suffix = dialect === 'pg' ? '-pg' : '';
  const files = collectTemplateFiles(infraDir, dialect, fsMod);
  const dir = path.join(outRoot, `coder-compose-template-${version}${suffix}`);
  fsMod.rmSync(dir, { recursive: true, force: true });
  fsMod.mkdirSync(dir, { recursive: true });

  for (const { rel, abs } of files) {
    const dest = path.join(dir, ...rel.split('/'));
    fsMod.mkdirSync(path.dirname(dest), { recursive: true });
    if (dialect === 'pg') {
      const text = fsMod.readFileSync(abs, 'utf8');
      const transformed = transformForPg(rel, text);
      fsMod.writeFileSync(dest, transformed ?? text, 'utf-8');
    } else {
      fsMod.copyFileSync(abs, dest);
    }
  }

  const info = {
    name: 'coder-compose-template',
    version,
    dialect: dialect === 'pg' ? 'postgres' : 'mysql',
    generatedAt: new Date().toISOString(),
    fileCount: files.length,
    source: 'low-memory-micro-services/infrastructure',
  };
  fsMod.writeFileSync(path.join(dir, 'TEMPLATE_INFO.json'), JSON.stringify(info, null, 2) + '\n', 'utf-8');
  return { dir, fileCount: files.length, dialect };
}

module.exports = {
  collectTemplateFiles,
  buildComposeTemplate,
  transformForPg,
  transformInfraCompose,
  transformAppCompose,
  transformEnvFile,
  transformProdOverlay,
  DIALECTS,
};
