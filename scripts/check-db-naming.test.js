'use strict';

/**
 * Regression tests for `scripts/check-db-naming.js`.
 *
 * The repository already uses the Node built-in test runner (see
 * `scripts/setup-wizard.test.js` and the `test:scripts` npm script), so this
 * file follows the same convention:
 *
 *   node --test scripts/check-db-naming.test.js
 *
 * The gate command `node scripts/check-db-naming.js` keeps its default
 * behaviour and exit codes; the tests only exercise the exported helpers.
 */

const { test } = require('node:test');
const assert = require('node:assert/strict');

const {
  VALID_PREFIXES,
  SKIP_IN_SOURCE,
  NON_TABLE_TOKENS_AFTER_UPDATE,
  UPDATE_CLAUSE_INTRODUCERS,
  isValidTableName,
  precedingKeyword,
  isOnUpdateClause,
  checkSqlString,
  checkSqlFile,
  checkRustFile,
  extractSqlxStrings,
} = require('./check-db-naming');

/** Table names captured for a SQL snippet, in scan order. */
function tablesOf(sql) {
  return checkSqlString(sql).map((v) => v.table);
}

/** SQL keyword index used by the clause-position unit tests. */
function updateIndex(text) {
  return text.lastIndexOf('UPDATE');
}

// ---------------------------------------------------------------------------
// 1. VALID_PREFIXES integrity (the sparse-array hole that used to exist)
// ---------------------------------------------------------------------------

test('VALID_PREFIXES has no holes and only string entries', () => {
  assert.equal(VALID_PREFIXES.length, 7);
  for (let i = 0; i < VALID_PREFIXES.length; i += 1) {
    assert.equal(typeof VALID_PREFIXES[i], 'string', `index ${i} must be a string`);
  }
  // A hole would let `name.startsWith(undefined)` compare against "undefined".
  assert.equal(VALID_PREFIXES.includes(undefined), false);
  assert.deepEqual(VALID_PREFIXES, [
    'iam_', 'audit_', 'token_', 'ntf_', 'cfg_', 'file_', 'prue_',
  ]);
});

test('isValidTableName accepts every registered prefix and rejects bare names', () => {
  for (const prefix of VALID_PREFIXES) {
    assert.equal(isValidTableName(`${prefix}users`), true, `${prefix}users must pass`);
  }
  assert.equal(isValidTableName('users'), false);
  assert.equal(isValidTableName('undefined'), false);
  assert.equal(isValidTableName('undefined_users'), false);
});

// ---------------------------------------------------------------------------
// 2. PostgreSQL upsert: `ON CONFLICT ... DO UPDATE SET` must not report `SET`
// ---------------------------------------------------------------------------

test('PG: ON CONFLICT DO UPDATE SET is not a bare table name', () => {
  const sql = [
    'INSERT INTO iam_users (id, name) VALUES ($1, $2)',
    'ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name;',
  ].join('\n');
  assert.deepEqual(tablesOf(sql), []);
  assert.deepEqual(checkSqlString(sql), []);
});

test('PG: ON CONFLICT ON CONSTRAINT ... DO UPDATE SET is not a bare table name', () => {
  const sql = [
    'INSERT INTO iam_users (id, name) VALUES ($1, $2)',
    'ON CONFLICT ON CONSTRAINT iam_users_pkey DO UPDATE SET name = EXCLUDED.name,',
    '  updated_at = now();',
  ].join('\n');
  assert.deepEqual(tablesOf(sql), []);
});

test('PG: ON CONFLICT DO NOTHING yields no violation', () => {
  const sql = 'INSERT INTO iam_users (id) VALUES ($1) ON CONFLICT DO NOTHING;';
  assert.deepEqual(tablesOf(sql), []);
});

test('SET is registered as a non-table token after UPDATE', () => {
  assert.equal(NON_TABLE_TOKENS_AFTER_UPDATE.has('set'), true);
});

// ---------------------------------------------------------------------------
// 3. The exact corpus that used to produce `["pg_catalog","SET"]`
// ---------------------------------------------------------------------------

test('PG corpus from the bug report reports zero violations', () => {
  const sql = [
    "INSERT INTO iam_users (id, name) VALUES (1, 'a')",
    'ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name;',
    '',
    'SELECT COUNT(*) FROM pg_catalog.pg_indexes WHERE schemaname = current_schema();',
  ].join('\n');
  assert.deepEqual(checkSqlString(sql), []);
});

test('PG system catalogs and built-ins are skipped', () => {
  const identifiers = [
    'pg_catalog',
    'pg_indexes',
    'pg_tables',
    'pg_class',
    'pg_namespace',
    'pg_stat_activity',
    'pg_stat_user_tables',
    'pg_advisory_lock',
    'pg_try_advisory_xact_lock',
    'current_schema',
    'current_database',
    'information_schema',
  ];
  for (const identifier of identifiers) {
    assert.equal(SKIP_IN_SOURCE.has(identifier), true, `${identifier} must be whitelisted`);
    assert.deepEqual(
      tablesOf(`SELECT * FROM ${identifier} WHERE x = current_schema();`),
      [],
      `FROM ${identifier} must not be reported`,
    );
  }
  // Schema-qualified catalog access is the shape migrations actually use.
  assert.deepEqual(tablesOf('SELECT * FROM pg_catalog.pg_tables;'), []);
  assert.deepEqual(tablesOf('SELECT * FROM pg_catalog.pg_class;'), []);
});

// ---------------------------------------------------------------------------
// 4. Real UPDATE statements must still be checked (no over-fix regression)
// ---------------------------------------------------------------------------

test('bare UPDATE on an unprefixed table is still a violation', () => {
  assert.deepEqual(tablesOf('UPDATE users SET a = 1;'), ['users']);
});

test('bare UPDATE is still a violation after a PG-style multi-line statement', () => {
  const sql = [
    "INSERT INTO iam_users (id, name) VALUES (1, 'a')",
    'ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name;',
    '',
    'UPDATE users SET a = 1;',
  ].join('\n');
  assert.deepEqual(tablesOf(sql), ['users']);
  // The offending UPDATE sits on line 4.
  assert.deepEqual(checkSqlString(sql), [{ table: 'users', line: 4 }]);
});

test('prefixed UPDATE stays clean', () => {
  assert.deepEqual(tablesOf('UPDATE iam_users SET name = $1 WHERE id = $2;'), []);
});

test('a table whose name merely starts with "set" is not skipped', () => {
  assert.deepEqual(tablesOf('UPDATE settings SET a = 1;'), ['settings']);
  assert.deepEqual(tablesOf('UPDATE set_log SET a = 1;'), ['set_log']);
});

// ---------------------------------------------------------------------------
// 5. Clause-form UPDATE (ON ... / KEY ... / FOR ...) is not a table position
// ---------------------------------------------------------------------------

test('precedingKeyword returns the keyword right before UPDATE', () => {
  assert.equal(precedingKeyword('... TIMESTAMP ON UPDATE', updateIndex('... TIMESTAMP ON UPDATE')), 'on');
  assert.equal(precedingKeyword('... KEY UPDATE', updateIndex('... KEY UPDATE')), 'key');
  assert.equal(precedingKeyword('... FOR UPDATE', updateIndex('... FOR UPDATE')), 'for');
  assert.equal(precedingKeyword('... DO UPDATE', updateIndex('... DO UPDATE')), 'do');
  assert.equal(precedingKeyword('UPDATE x', 0), '');
});

test('isOnUpdateClause covers ON UPDATE / KEY UPDATE / FOR UPDATE', () => {
  const onDefault = 'CREATE TABLE iam_x (updated_at TIMESTAMP ON UPDATE';
  assert.equal(isOnUpdateClause(onDefault, updateIndex(onDefault)), true);

  const onDuplicateKey = 'INSERT INTO iam_x (id) VALUES (1) ON DUPLICATE KEY UPDATE';
  assert.equal(isOnUpdateClause(onDuplicateKey, updateIndex(onDuplicateKey)), true);

  const forUpdate = 'SELECT * FROM iam_x WHERE id = 1 FOR UPDATE';
  assert.equal(isOnUpdateClause(forUpdate, updateIndex(forUpdate)), true);

  const plainUpdate = 'UPDATE iam_x';
  assert.equal(isOnUpdateClause(plainUpdate, updateIndex(plainUpdate)), false);

  const pgUpsert = 'ON CONFLICT (id) DO UPDATE';
  assert.equal(
    isOnUpdateClause(pgUpsert, updateIndex(pgUpsert)),
    false,
    'PG upsert is handled by the SET token rule, not by the clause keyword rule',
  );

  assert.deepEqual([...UPDATE_CLAUSE_INTRODUCERS].sort(), ['for', 'key', 'on']);
});

test('MySQL upsert, column default and row locking keep reporting nothing', () => {
  const corpus = [
    'INSERT INTO iam_users (id, name) VALUES (?, ?) ON DUPLICATE KEY UPDATE name = VALUES(name);',
    'ALTER TABLE iam_users ADD COLUMN updated_at TIMESTAMP ON UPDATE CURRENT_TIMESTAMP;',
    'SELECT id FROM iam_users WHERE id = ? FOR UPDATE;',
    'SELECT id FROM iam_users WHERE id = ? FOR UPDATE NOWAIT;',
    'SELECT id FROM iam_users WHERE id = ? FOR UPDATE SKIP LOCKED;',
    'SELECT id FROM iam_users WHERE id = ? FOR NO KEY UPDATE;',
  ];
  for (const sql of corpus) {
    assert.deepEqual(tablesOf(sql), [], `unexpected violation in: ${sql}`);
  }
});

test('a long ON DUPLICATE KEY UPDATE prefix does not break the clause rule', () => {
  const sql = [
    "INSERT INTO iam_users (id, name, email, created_at, updated_at, deleted_at)",
    "VALUES (?, ?, ?, now(), now(), 0)",
    "ON DUPLICATE KEY UPDATE name = VALUES(name), email = VALUES(email);",
  ].join('\n');
  assert.deepEqual(tablesOf(sql), []);
});

// ---------------------------------------------------------------------------
// 6. Existing behaviour: all other bare-table shapes are still caught
// ---------------------------------------------------------------------------

test('unprefixed FROM / JOIN / INTO / DELETE FROM are still violations', () => {
  assert.deepEqual(tablesOf('SELECT * FROM users;'), ['users']);
  assert.deepEqual(tablesOf('SELECT * FROM iam_users u JOIN roles r ON r.id = u.role_id;'), ['roles']);
  assert.deepEqual(tablesOf('INSERT INTO users (id) VALUES (1);'), ['users']);
  // `DELETE FROM users` matches two patterns; the table is reported once.
  assert.deepEqual(tablesOf('DELETE FROM users WHERE id = 1;'), ['users']);
});

test('prefixed FROM / JOIN / INTO / DELETE FROM stay clean', () => {
  const corpus = [
    'SELECT * FROM iam_users;',
    'SELECT * FROM iam_users u JOIN iam_roles r ON r.id = u.role_id;',
    'INSERT INTO iam_users (id) VALUES ($1);',
    'DELETE FROM iam_users WHERE id = $1;',
  ];
  for (const sql of corpus) {
    assert.deepEqual(tablesOf(sql), [], `unexpected violation in: ${sql}`);
  }
});

// ---------------------------------------------------------------------------
// 7. Comment stripping, line numbers and sqlx extraction
// ---------------------------------------------------------------------------

test('checkSqlFile strips both comment styles', () => {
  const sql = [
    '-- FROM users (commented out)',
    '/* UPDATE users SET a = 1; */',
    'SELECT * FROM iam_users;',
  ].join('\n');
  assert.deepEqual(checkSqlFile(sql), []);
});

test('checkSqlFile reports 1-based line numbers', () => {
  const sql = [
    'CREATE TABLE iam_users (id BIGINT PRIMARY KEY);',
    '',
    'INSERT INTO users (id) VALUES (1);',
  ].join('\n');
  assert.deepEqual(checkSqlFile(sql), [{ table: 'users', line: 3 }]);
});

test('checkRustFile maps violations back to the Rust source line', () => {
  const rust = [
    'async fn load(pool: &PgPool) -> Result<()> {', // 1
    '    let rows = sqlx::query("SELECT * FROM users WHERE id = $1")', // 2
    '        .fetch_all(pool)', // 3
    '        .await?;', // 4
    '    Ok(())', // 5
    '}', // 6
  ].join('\n');
  assert.deepEqual(checkRustFile(rust), [{ table: 'users', line: 2 }]);
});

test('checkRustFile keeps PG upserts in sqlx strings clean', () => {
  const rust = [
    'async fn upsert(pool: &PgPool) -> Result<()> {', // 1
    '    sqlx::query(', // 2
    '        "INSERT INTO iam_users (id, name) VALUES ($1, $2) \\', // 3
    '         ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name",', // 4
    '    )', // 5
    '    .execute(pool)', // 6
    '    .await?;', // 7
    '    Ok(())', // 8
    '}', // 9
  ].join('\n');
  assert.deepEqual(checkRustFile(rust), []);
});

test('checkRustFile reports several literals in source order', () => {
  const rust = [
    'async fn load(pool: &PgPool) {', // 1
    '    sqlx::query("SELECT * FROM users").fetch_one(pool).await;', // 2
    '    sqlx::query("SELECT * FROM roles").fetch_one(pool).await;', // 3
    '}', // 4
  ].join('\n');
  assert.deepEqual(checkRustFile(rust), [
    { table: 'users', line: 2 },
    { table: 'roles', line: 3 },
  ]);
});

test('extractSqlxStrings finds both literal shapes without duplicates', () => {
  const rust = [
    'sqlx::query("SELECT 1")',
    'sqlx::query_as(&"SELECT 2")',
    'sqlx::query_as("SELECT 3")',
  ].join('\n');
  const found = extractSqlxStrings(rust).map((entry) => entry.sql);
  // Source order, and the overlapping patterns must not duplicate a literal.
  assert.deepEqual(found, ['SELECT 1', 'SELECT 2', 'SELECT 3']);
});

test('extractSqlxStrings also covers turbofish calls', () => {
  const rust = [
    'sqlx::query_as::<_, Announcement>("SELECT 1")',
    'sqlx::query_as::<_, Vec<Row>>("SELECT 2")',
    'sqlx::query_as::<_, Row>(\n    "SELECT 3"\n)',
  ].join('\n');
  const found = extractSqlxStrings(rust).map((entry) => entry.sql);
  assert.deepEqual(found, ['SELECT 1', 'SELECT 2', 'SELECT 3']);
});

test('checkRustFile scans turbofish sqlx literals', () => {
  const rust = [
    'async fn load(pool: &PgPool) {', // 1
    '    sqlx::query_as::<_, Row>(', // 2
    '        "SELECT * FROM users"', // 3
    '    ).fetch_all(pool).await;', // 4
    '}', // 5
  ].join('\n');
  assert.deepEqual(checkRustFile(rust), [{ table: 'users', line: 3 }]);
});

test('table names are compared case-insensitively', () => {
  assert.deepEqual(tablesOf('select * from users;'), ['users']);
  assert.deepEqual(tablesOf('SELECT * FROM PG_CATALOG.PG_INDEXES;'), []);
  assert.deepEqual(tablesOf('select * from IAM_USERS;'), []);
});
