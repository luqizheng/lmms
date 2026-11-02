#!/usr/bin/env node
'use strict';

const fs = require('fs');
const path = require('path');

/**
 * Service table prefixes. Every business table must start with one of these so
 * that services sharing one physical database stay logically isolated.
 *
 * Never leave holes (sparse entries) in this array: `Array.prototype.some`
 * skips holes, and `String.prototype.startsWith(undefined)` silently compares
 * against the literal string "undefined".
 */
const VALID_PREFIXES = [
  'iam_',
  'audit_',
  'token_',
  'ntf_',
  'cfg_',
  'file_',
  'prue_',
];

/**
 * Identifiers that are never a business table, so the prefix rule does not
 * apply. Two families live here:
 *
 *  1. Engine catalogs / metadata tables (`information_schema`, `pg_catalog`, ...).
 *     PostgreSQL reserves the `pg_` prefix for system catalogs and system views,
 *     therefore none of them can ever carry a service prefix.
 *  2. Built-in functions and keyword-like tokens that the regex scanner reads as
 *     if they were table names (`current_schema()`, `current_timestamp`, ...).
 */
const SKIP_IN_SOURCE = new Set([
  // MySQL / SQLite / ANSI metadata
  'information_schema',
  'mysql',
  'performance_schema',
  'sys',
  'sqlite_master',
  'pragma_table_info',
  // Built-in functions (MySQL + PostgreSQL)
  'current_timestamp',
  'current_schema',
  'current_database',
  'current_catalog',
  'current_user',
  'session_user',
  // PostgreSQL system catalogs & catalog views
  'pg_catalog',
  'pg_indexes',
  'pg_tables',
  'pg_class',
  'pg_namespace',
  'pg_index',
  'pg_attribute',
  'pg_type',
  'pg_constraint',
  'pg_proc',
  'pg_description',
  'pg_depend',
  'pg_attrdef',
  'pg_rewrite',
  'pg_inherits',
  'pg_enum',
  'pg_range',
  'pg_trigger',
  'pg_policies',
  'pg_rules',
  'pg_views',
  'pg_matviews',
  'pg_sequences',
  'pg_extension',
  'pg_toast',
  'pg_am',
  'pg_locks',
  'pg_roles',
  'pg_database',
  'pg_settings',
  'pg_stat_activity',
  'pg_stat_user_tables',
  'pg_stat_statements',
  'pg_advisory_lock',
  'pg_advisory_xact_lock',
  'pg_try_advisory_lock',
  'pg_try_advisory_xact_lock',
]);

/**
 * `UPDATE` branch is kept in its own constant so the scanner can tell which
 * capture group it came from: only `UPDATE <token>` can swallow the `SET`
 * keyword of a PostgreSQL upsert (`... ON CONFLICT (id) DO UPDATE SET col = ...`).
 */
const UPDATE_PATTERN = /(?<![`\w])UPDATE\s+([a-z_][a-z0-9_]*)/gi;

/**
 * Tokens that can legally follow `UPDATE` but can never be a table name there.
 * `SET` is a reserved SQL keyword: it appears as the assignment keyword of a
 * real update (`UPDATE t SET c = ...`) and as the clause keyword of a
 * PostgreSQL upsert (`DO UPDATE SET ...`). In the latter case the token right
 * after `UPDATE` *is* `SET`, which must not be read as a table name.
 */
const NON_TABLE_TOKENS_AFTER_UPDATE = new Set(['set']);

/**
 * Keywords that may directly precede `UPDATE` while the following token is not
 * a table name:
 *
 *   - `ON`   -> `... ON UPDATE CURRENT_TIMESTAMP` (column default, MySQL)
 *   - `KEY`  -> `... ON DUPLICATE KEY UPDATE <col> = ...` (MySQL upsert)
 *   - `FOR`  -> `... FOR UPDATE [OF <table>] [NOWAIT]` (row locking, MySQL + PG)
 *
 * The lookup uses the *immediately preceding keyword* instead of a fixed-width
 * look-behind window, so it cannot break on long statements.
 */
const UPDATE_CLAUSE_INTRODUCERS = new Set(['on', 'key', 'for']);

const TABLE_PATTERNS = [
  /(?<![`\w])(?:FROM|JOIN)\s+([a-z_][a-z0-9_]*)/gi,
  /(?<![`\w])INTO\s+([a-z_][a-z0-9_]*)/gi,
  UPDATE_PATTERN,
  /(?<![`\w])DELETE\s+FROM\s+([a-z_][a-z0-9_]*)/gi,
];

/**
 * `sqlx::query*("...")` literals. Group 1 captures the call prefix (up to and
 * including the opening quote) so the literal's own start offset is exact,
 * group 2 captures the SQL text.
 *
 * The optional turbofish (`::<_, Row>`) is part of the prefix, otherwise
 * `sqlx::query_as::<_, Row>("SELECT ...")` would not be scanned at all.
 */
const SQLX_PATTERNS = [
  /(sqlx::query(?:_as)?(?:::<[^<>]*(?:<[^<>]*>[^<>]*)*>)?\s*\(\s*&?)"([^"]+)"/g,
];

/**
 * Returns true when `name` already carries a registered service prefix.
 *
 * SQL identifiers are compared case-insensitively, so the name is lower-cased
 * before the prefix comparison (`IAM_USERS` is the same table as `iam_users`).
 *
 * @param {string} name Table name as captured from SQL.
 * @returns {boolean} True when the name is prefixed (and therefore accepted).
 */
function isValidTableName(name) {
  const normalized = name.toLowerCase();
  return VALID_PREFIXES.some((prefix) => normalized === prefix || normalized.startsWith(prefix));
}

/**
 * Returns the last SQL keyword that appears directly before `index`.
 *
 * @param {string} text Full SQL text being scanned.
 * @param {number} index Position inside `text`.
 * @returns {string} Lower-cased keyword, or an empty string when there is none.
 */
function precedingKeyword(text, index) {
  const before = text.slice(0, index);
  const matched = before.match(/([A-Za-z_][A-Za-z0-9_]*)\s*$/);
  return matched ? matched[1].toLowerCase() : '';
}

/**
 * Returns true when the `UPDATE` keyword at `matchIndex` is part of a clause in
 * which the following token is not a table name:
 *
 *   - `... ON UPDATE CURRENT_TIMESTAMP`            (column default, MySQL)
 *   - `... ON DUPLICATE KEY UPDATE <column> = ...` (MySQL upsert)
 *   - `... FOR UPDATE [OF <table>] [NOWAIT]`       (row-locking clause, MySQL + PG)
 *
 * PostgreSQL's `ON CONFLICT ... DO UPDATE SET ...` is handled separately by
 * {@link NON_TABLE_TOKENS_AFTER_UPDATE}: the keyword in front of `UPDATE` is
 * `DO`, which is not an introducer, and the token after `UPDATE` is `SET`.
 *
 * @param {string} text Full SQL text being scanned.
 * @param {number} matchIndex Index of the `UPDATE` keyword inside `text`.
 * @returns {boolean} True when this `UPDATE` does not introduce a table name.
 */
function isOnUpdateClause(text, matchIndex) {
  return UPDATE_CLAUSE_INTRODUCERS.has(precedingKeyword(text, matchIndex));
}

/**
 * Scans one SQL string for table names that lack a service prefix.
 *
 * Violations are de-duplicated per (line, table): several patterns can match
 * the same table name (`DELETE FROM t` matches both the `FROM` and the
 * `DELETE FROM` pattern), and one table must be reported only once.
 *
 * @param {string} sql SQL text (comments already stripped by the caller).
 * @param {number} [lineOffset=0] Zero-based line offset of `sql` in its source file.
 * @returns {Array<{table: string, line: number}>} Violations with 1-based lines.
 */
function checkSqlString(sql, lineOffset = 0) {
  const violations = [];
  const seen = new Set();
  for (const pattern of TABLE_PATTERNS) {
    const isUpdateBranch = pattern === UPDATE_PATTERN;
    let m;
    pattern.lastIndex = 0;
    while ((m = pattern.exec(sql)) !== null) {
      const table = m[1];
      // `DO UPDATE SET col = ...` — `SET` is the clause keyword, not a table.
      // Real updates (`UPDATE <table> SET ...`) keep the table name in m[1] and
      // are still reported.
      if (isUpdateBranch && NON_TABLE_TOKENS_AFTER_UPDATE.has(table.toLowerCase())) continue;
      if (isValidTableName(table)) continue;
      if (SKIP_IN_SOURCE.has(table.toLowerCase())) continue;
      if (isUpdateBranch && isOnUpdateClause(sql, m.index)) continue;
      const line = lineOffset + sql.slice(0, m.index).split('\n').length;
      const key = `${line}:${table.toLowerCase()}`;
      if (seen.has(key)) continue;
      seen.add(key);
      violations.push({ table, line });
    }
  }
  return violations;
}

/**
 * Extracts inline SQL held in `sqlx::query*("...")` string literals.
 *
 * Results are returned in source order and de-duplicated by offset, so the
 * violations reported for a Rust file follow the file.
 *
 * @param {string} code Rust source.
 * @returns {Array<{sql: string, offset: number}>} SQL text with the char offset
 *   of the literal's first character (not of the `sqlx::query` call).
 */
function extractSqlxStrings(code) {
  const results = [];
  const seen = new Set();
  for (const pattern of SQLX_PATTERNS) {
    let m;
    pattern.lastIndex = 0;
    while ((m = pattern.exec(code)) !== null) {
      if (seen.has(m.index)) continue;
      seen.add(m.index);
      // `m[1]` is the call prefix including the opening quote, so the literal
      // content starts right after it.
      results.push({ sql: m[2], offset: m.index + m[1].length });
    }
  }
  results.sort((a, b) => a.offset - b.offset);
  return results;
}

/**
 * Scans a Rust source file for violating table names inside sqlx queries.
 *
 * @param {string} content Rust source.
 * @returns {Array<{table: string, line: number}>} Violations with 1-based lines.
 */
function checkRustFile(content) {
  const violations = [];
  const sqlxStrings = extractSqlxStrings(content);
  for (const { sql, offset } of sqlxStrings) {
    const lineOffset = content.slice(0, offset).split('\n').length - 1;
    violations.push(...checkSqlString(sql, lineOffset));
  }
  return violations;
}

/**
 * Scans a `.sql` file, with block comments and `--` line comments removed first.
 *
 * @param {string} content SQL file content.
 * @returns {Array<{table: string, line: number}>} Violations with 1-based lines.
 */
function checkSqlFile(content) {
  const stripped = content
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/--[^\n]*/g, '');
  return checkSqlString(stripped, 0);
}

/**
 * Recursively lists files under `dir` matching any of `extensions`.
 *
 * @param {string} dir Directory to walk.
 * @param {string[]} extensions Accepted suffixes (e.g. `['.rs']`).
 * @returns {string[]} Absolute file paths.
 */
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

/**
 * Runs the gate over the repository: exits 0 on PASS, 1 on violations.
 *
 * @returns {Promise<void>} Resolves after the exit code is set.
 */
async function main() {
  const errors = [];
  const repoRoot = path.resolve(__dirname, '..');

  const rustFiles = walkDir(path.join(repoRoot, 'services'), ['.rs']);
  const migrationFiles = [
    ...walkDir(path.join(repoRoot, 'services'), ['.sql']),
    ...walkDir(path.join(repoRoot, 'apps'), ['.sql']),
    ...walkDir(path.join(repoRoot, 'scripts', 'migrations'), ['.sql']),
  ];

  for (const file of rustFiles) {
    const content = fs.readFileSync(file, 'utf-8');
    const violations = checkRustFile(content);
    for (const v of violations) {
      errors.push({ file: path.relative(repoRoot, file), ...v });
    }
  }

  for (const file of migrationFiles) {
    const content = fs.readFileSync(file, 'utf-8');
    const violations = checkSqlFile(content);
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

module.exports = {
  VALID_PREFIXES,
  SKIP_IN_SOURCE,
  TABLE_PATTERNS,
  UPDATE_PATTERN,
  NON_TABLE_TOKENS_AFTER_UPDATE,
  UPDATE_CLAUSE_INTRODUCERS,
  SQLX_PATTERNS,
  isValidTableName,
  precedingKeyword,
  isOnUpdateClause,
  checkSqlString,
  checkSqlFile,
  checkRustFile,
  extractSqlxStrings,
  walkDir,
  main,
};

if (require.main === module) {
  main().catch(err => {
    console.error(err);
    process.exit(1);
  });
}
