#!/usr/bin/env node
// scripts/check-env.mjs
import process from 'node:process';
import { runChecks, renderTable } from './lib/EnvChecker.mjs';

const args = new Set(process.argv.slice(2));
const jsonMode = args.has('--json');

const NO_COLOR = 'NO_COLOR' in process.env;
const useColor = !NO_COLOR && process.stdout.isTTY;
const isWindowsLegacy = process.platform === 'win32' && !('WT_SESSION' in process.env) && !process.env.TERM?.includes('xterm');

const c = {
  reset: useColor && !isWindowsLegacy ? '\x1b[0m' : '',
  red: useColor && !isWindowsLegacy ? '\x1b[31m' : '',
  green: useColor && !isWindowsLegacy ? '\x1b[32m' : '',
  gray: useColor && !isWindowsLegacy ? '\x1b[90m' : '',
  bold: useColor && !isWindowsLegacy ? '\x1b[1m' : '',
};

function colorize(text, color) {
  return color ? `${color}${text}${c.reset}` : text;
}

const results = await runChecks();

if (jsonMode) {
  const payload = results.map((r) => ({
    tool: r.name,
    required: typeof r.required === 'string' ? r.required : `>=${r.required.major}.${r.required.minor}.${r.required.patch}`,
    detected: r.detected === null
      ? null
      : typeof r.detected === 'string'
        ? r.detected
        : `${r.detected.major}.${r.detected.minor}.${r.detected.patch}`,
    status: r.status,
    ...(r.hint ? { hint: r.hint } : {}),
  }));
  process.stdout.write(JSON.stringify(payload, null, 2) + '\n');
} else {
  const table = renderTable(results);
  const lines = table.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const row = results[i - 2];
    if (i >= 2 && row) {
      const status = row.status.toUpperCase();
      if (status === 'OK') {
        process.stdout.write(colorize(lines[i], c.green) + '\n');
      } else if (status === 'FAIL') {
        process.stdout.write(colorize(c.bold + c.red + lines[i] + c.reset) + '\n');
      } else if (status === 'SKIP') {
        process.stdout.write(colorize(lines[i], c.gray) + '\n');
      } else {
        process.stdout.write(lines[i] + '\n');
      }
    } else {
      process.stdout.write(lines[i] + '\n');
    }
  }

  const failed = results.filter((r) => r.status === 'fail');
  if (failed.length > 0) {
    process.stdout.write('\n');
    process.stdout.write(`Result: ${failed.length} check(s) failed.\n`);
    for (const f of failed) {
      process.stdout.write(`- ${f.label}: ${f.hint}\n`);
    }
  } else {
    process.stdout.write('\n');
    process.stdout.write(colorize('Result: all checks passed.', c.green) + '\n');
  }
}

const failedCount = results.filter((r) => r.status === 'fail').length;
process.exit(failedCount > 0 ? 1 : 0);
