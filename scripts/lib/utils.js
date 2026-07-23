// utils.js
// 共享工具：路径、文件 IO
//
// 颜色 / spinner / prompt 相关工具已迁移到 ./ui.js 和 ./prompt.js（基于 chalk / ora / prompts）
// 老的 log.*、paint()、COLORS 已删除。下游请改用：
//   const { chalk, success, warn, error, info, section, banner, spinner } = require('./lib/ui');
//   const { input, confirm, select } = require('./lib/prompt');
const path = require('node:path');

const ROOT = process.env.CODER_ROOT || path.resolve(__dirname, '..', '..');

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

module.exports = { ROOT, reportsDir, runDir, formatTimestamp, findLatestRun };
