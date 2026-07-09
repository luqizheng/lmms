// utils.js
// 共享工具：颜色、路径、文件 IO
const path = require('node:path');

const ROOT = process.env.CODER_ROOT || path.resolve(__dirname, '..', '..');

const COLORS = {
  reset: '\x1b[0m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  magenta: '\x1b[35m',
  cyan: '\x1b[36m',
  gray: '\x1b[90m',
  bold: '\x1b[1m',
};

function paint(color, text) {
  if (!process.stdout.isTTY) return text;
  return `${COLORS[color]}${text}${COLORS.reset}`;
}

const log = {
  red:    (t) => process.stdout.write(paint('red', t) + '\n'),
  green:  (t) => process.stdout.write(paint('green', t) + '\n'),
  yellow: (t) => process.stdout.write(paint('yellow', t) + '\n'),
  blue:   (t) => process.stdout.write(paint('blue', t) + '\n'),
  magenta:(t) => process.stdout.write(paint('magenta', t) + '\n'),
  cyan:   (t) => process.stdout.write(paint('cyan', t) + '\n'),
  gray:   (t) => process.stdout.write(paint('gray', t) + '\n'),
  bold:   (t) => process.stdout.write(paint('bold', t) + '\n'),
  plain:  (t) => process.stdout.write(t + '\n'),
};

function reportsDir() {
  return path.join(ROOT, 'reports');
}

function runDir(timestamp) {
  const ts = timestamp || formatTimestamp(new Date());
  return path.join(reportsDir(), `run-${ts}`);
}

function formatTimestamp(d) {
  const pad = (n) => String(n).padStart(2, '0');
  return (
    d.getFullYear() +
    pad(d.getMonth() + 1) +
    pad(d.getDate()) +
    '-' +
    pad(d.getHours()) +
    pad(d.getMinutes()) +
    pad(d.getSeconds())
  );
}

function findLatestRun() {
  const dir = reportsDir();
  if (!require('node:fs').existsSync(dir)) return null;
  const entries = require('node:fs').readdirSync(dir)
    .filter((n) => n.startsWith('run-'))
    .sort()
    .reverse();
  if (!entries.length) return null;
  return path.join(dir, entries[0]);
}

module.exports = { ROOT, COLORS, paint, log, reportsDir, runDir, formatTimestamp, findLatestRun };
