'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { parseSpec } = require('../lib/spec.js');
const { EXIT } = require('../lib/errors.js');

const SPEC = {
  flags: {
    '--version': { type: 'value', desc: 'target version' },
    '--dry-run': { type: 'flag', desc: 'preview only' },
  },
  positional: { name: 'version', desc: 'positional version', required: false },
};

test('known value flag is parsed and its value captured', () => {
  const r = parseSpec(SPEC, ['--version', '1.7.0']);
  assert.equal(r.flags['--version'], '1.7.0');
  assert.deepEqual(r.positional, []);
});

test('known boolean flag is parsed as true', () => {
  const r = parseSpec(SPEC, ['--dry-run']);
  assert.equal(r.flags['--dry-run'], true);
});

test('unknown flag is rejected with USAGE exit code', () => {
  assert.throws(
    () => parseSpec(SPEC, ['--totally-bogus']),
    (err) => {
      assert.equal(err.exitCode, EXIT.USAGE);
      assert.equal(err.code, 'UNKNOWN_FLAG');
      assert.match(err.message, /--totally-bogus/);
      return true;
    },
  );
});

test('unknown flag is rejected even when it follows a valid flag', () => {
  assert.throws(
    () => parseSpec(SPEC, ['--dry-run', '--nope']),
    (err) => err.code === 'UNKNOWN_FLAG',
  );
});

test('value flag without a value is rejected with MISSING_FLAG_VALUE', () => {
  assert.throws(
    () => parseSpec(SPEC, ['--version']),
    (err) => {
      assert.equal(err.exitCode, EXIT.USAGE);
      assert.equal(err.code, 'MISSING_FLAG_VALUE');
      assert.match(err.message, /--version/);
      return true;
    },
  );
});

test('value flag followed by another flag is treated as missing value', () => {
  assert.throws(
    () => parseSpec(SPEC, ['--version', '--dry-run']),
    (err) => err.code === 'MISSING_FLAG_VALUE',
  );
});

test('positional argument is captured in order', () => {
  const r = parseSpec(SPEC, ['1.7.0']);
  assert.deepEqual(r.positional, ['1.7.0']);
});

test('too many positional arguments are rejected', () => {
  assert.throws(
    () => parseSpec(SPEC, ['a', 'b']),
    (err) => {
      assert.equal(err.exitCode, EXIT.USAGE);
      assert.equal(err.code, 'TOO_MANY_POSITIONAL');
      return true;
    },
  );
});

test('required positional missing is rejected', () => {
  const spec = { flags: {}, positional: { name: 'version', required: true } };
  assert.throws(
    () => parseSpec(spec, []),
    (err) => {
      assert.equal(err.exitCode, EXIT.USAGE);
      assert.equal(err.code, 'MISSING_POSITIONAL');
      return true;
    },
  );
});

test('variadic positional accepts any number of values', () => {
  const spec = { flags: {}, positional: { name: 'services', variadic: true } };
  const r = parseSpec(spec, ['api-gateway', 'iam-service']);
  assert.deepEqual(r.positional, ['api-gateway', 'iam-service']);
});

test('spec without positional section rejects any positional argument', () => {
  assert.throws(
    () => parseSpec({ flags: {} }, ['oops']),
    (err) => err.code === 'TOO_MANY_POSITIONAL',
  );
});

test('double-dash separator passes the rest through as positionals', () => {
  const r = parseSpec(SPEC, ['--dry-run', '--', '--not-a-flag']);
  assert.deepEqual(r.positional, ['--not-a-flag']);
  assert.equal(r.flags['--dry-run'], true);
});
