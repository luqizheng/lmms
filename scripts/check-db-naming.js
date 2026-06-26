#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

const VALID_PREFIXES = [
  'iam_', 'audit_', 'token_', 'ntf_', 'cfg_', 'file_', 'prue_',
];

const SKIP_IN_SOURCE = new Set([
  'information_schema',
  'mysql',
  'performance_schema',
  'sys',
  'current_timestamp',
  'sqlite_master',
  'pragma_table_info',
]);

const SQLX_PATTERNS = [
  /sqlx::query(?:_as)?\s*\(\s*"([^"]+)"/g,
  /sqlx::query(?:_as)?\s*\(\s*&?"([^"]+)"\s*\)/g,
];

const TABLE_PATTERNS = [
  /(?<![`\w])(?:FROM|JOIN)\s+([a-z_][a-z0-9_]*)/gi,
  /(?<![`\w])INTO\s+([a-z_][a-z0-9_]*)/gi,
  /(?<![`\w])UPDATE\s+([a-z_][a-z0-9_]*)/gi,
  /(?<![`\w])DELETE\s+FROM\s+([a-z_][a-z0-9_]*)/gi,
];

function isValidTableName(name) {
  return VALID_PREFIXES.some(p => name === p || name.startsWith(p));
}

function isOnUpdateClause(text, matchIndex) {
  const before = text.slice(Math.max(0, matchIndex - 4), matchIndex).toUpperCase();
  return /ON\s*$/.test(before);
}

function checkSqlString(sql, filePath, lineOffset) {
  const violations = [];
  for (const pattern of TABLE_PATTERNS) {
    let m;
    pattern.lastIndex = 0;
    while ((m = pattern.exec(sql)) !== null) {
      const table = m[1];
      if (isValidTableName(table)) continue;
      if (SKIP_IN_SOURCE.has(table.toLowerCase())) continue;
      if (isOnUpdateClause(sql, m.index)) continue;
      violations.push({
        table,
        line: lineOffset + sql.slice(0, m.index).split('\n').length - 1,
      });
    }
  }
  return violations;
}

function extractSqlxStrings(code) {
  const results = [];
  for (const pattern of SQLX_PATTERNS) {
    let m;
    pattern.lastIndex = 0;
    while ((m = pattern.exec(code)) !== null) {
      results.push({ sql: m[1], offset: m.index });
    }
  }
  return results;
}

function checkRustFile(content) {
  const violations = [];
  const sqlxStrings = extractSqlxStrings(content);
  for (const { sql, offset } of sqlxStrings) {
    const lineOffset = content.slice(0, offset).split('\n').length;
    const v = checkSqlString(sql, filePath, lineOffset);
    violations.push(...v);
  }
  return violations;
}

function checkSqlFile(content, filePath) {
  const stripped = content
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/--[^\n]*/g, '');
  return checkSqlString(stripped, filePath, 0);
}

function walkDir(dir, extensions) {
  const results = [];
  if (!fs.existsSync(dir)) return results;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory() && !entry.name.startsWith('.') && entry.name !== 'node_modules' && entry.name !== 'target') {
      results.push(...walkDir(full, extensions));
    } else if (entry.isFile() && extensions.some(ext => entry.name.endsWith(ext))) {
      results.push(full);
    }
  }
  return results;
}

let filePath = '';

async function main() {
  const errors = [];
  const repoRoot = path.resolve(__dirname, '..');

  const rustFiles = walkDir(path.join(repoRoot, 'services'), ['.rs']);
  const migrationFiles = [
    ...walkDir(path.join(repoRoot, 'services'), ['.sql']),
    ...walkDir(path.join(repoRoot, 'scripts', 'migrations'), ['.sql']),
  ];

  for (const file of rustFiles) {
    filePath = file;
    const content = fs.readFileSync(file, 'utf-8');
    const violations = checkRustFile(content);
    for (const v of violations) {
      errors.push({ file: path.relative(repoRoot, file), ...v });
    }
  }

  for (const file of migrationFiles) {
    filePath = file;
    const content = fs.readFileSync(file, 'utf-8');
    const violations = checkSqlFile(content, file);
    for (const v of violations) {
      errors.push({ file: path.relative(repoRoot, file), ...v });
    }
  }

  if (errors.length === 0) {
    console.log('PASS: no bare table names found in SQL contexts');
    process.exit(0);
  } else {
    console.error(`FAIL: ${errors.length} bare table name(s) found:`);
    for (const e of errors) {
      console.error(`  ${e.file}:${e.line} — table "${e.table}" has no prefix`);
    }
    process.exit(1);
  }
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});