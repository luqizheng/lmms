// scripts/lib/EnvChecker.mjs
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
import { parseVersion, isAtLeast } from './VersionParser.mjs';

const execFileP = promisify(execFile);

const isMain = import.meta.url === pathToFileURL(process.argv[1] ?? '').href;

const CHECKS = {
  node: {
    label: 'Node',
    required: { major: 18, minor: 0, patch: 0 },
    cmd: ['node', '-v'],
    parse: (out) => parseVersion(out.trim()),
    hint: 'install Node 18+ from https://nodejs.org',
  },
  docker: {
    label: 'Docker',
    required: { major: 20, minor: 10, patch: 0 },
    cmd: ['docker', '--version'],
    parse: (out) => parseVersion(out),
    hint: 'install Docker Desktop or docker-ce 20.10+',
  },
  powershell: {
    label: 'PowerShell',
    required: { major: 5, minor: 1, patch: 0 },
    cmdWindows: ['powershell.exe', '-NoProfile', '-Command', '$PSVersionTable.PSVersion.Major'],
    cmdUnix: ['pwsh', '-NoProfile', '-Command', '$PSVersionTable.PSVersion.ToString()'],
    parse: (out) => parseVersion(out.trim()),
    hint: 'install PowerShell 7+ from https://aka.ms/powershell',
  },
  git: {
    label: 'Git',
    required: { major: 2, minor: 30, patch: 0 },
    cmd: ['git', '--version'],
    parse: (out) => parseVersion(out),
    hint: 'install Git 2.30+ from https://git-scm.com',
  },
  rustc: {
    label: 'Rust',
    required: { major: 1, minor: 75, patch: 0 },
    cmd: ['rustc', '--version'],
    parse: (out) => parseVersion(out),
    hint: 'install via https://rustup.rs',
  },
  cargo: {
    label: 'Cargo',
    required: { major: 1, minor: 75, patch: 0 },
    cmd: ['cargo', '--version'],
    parse: (out) => parseVersion(out),
    hint: 'install via https://rustup.rs',
  },
};

async function runCmd(cmd) {
  try {
    const { stdout } = await execFileP(cmd[0], cmd.slice(1), { timeout: 5000, windowsHide: true });
    return { ok: true, stdout: stdout.toString() };
  } catch (err) {
    return { ok: false, code: err.code, stderr: err.stderr?.toString() ?? '' };
  }
}

async function detectOne(name, platform) {
  const spec = CHECKS[name];
  const cmd = spec.cmd
    ?? (platform === 'win32' ? spec.cmdWindows : spec.cmdUnix);
  const { ok, stdout } = await runCmd(cmd);
  if (!ok) {
    return { name, label: spec.label, required: spec.required, detected: null, status: 'fail', hint: spec.hint };
  }
  const detected = spec.parse(stdout);
  if (detected === null) {
    return { name, label: spec.label, required: spec.required, detected: null, status: 'fail', hint: `unparseable output: ${stdout.trim().slice(0, 80)}` };
  }
  const status = isAtLeast(detected, spec.required) ? 'ok' : 'fail';
  return { name, label: spec.label, required: spec.required, detected, status, hint: status === 'ok' ? '' : spec.hint };
}

async function detectMsvc() {
  if (process.platform !== 'win32') {
    return { name: 'msvc', label: 'MSVC v143', required: 'required', detected: null, status: 'skip', hint: 'Windows only' };
  }
  const where = await runCmd(['where', 'vswhere.exe']);
  if (!where.ok) {
    return { name: 'msvc', label: 'MSVC v143', required: 'required', detected: null, status: 'fail', hint: 'vswhere.exe not found; install VS Build Tools 2022' };
  }
  const vswherePath = where.stdout.trim().split(/\r?\n/)[0];
  const probe = await runCmd([
    vswherePath, '-latest',
    '-products', '*',
    '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64',
    '-property', 'installationPath',
  ]);
  const detected = probe.ok && probe.stdout.trim().length > 0 ? probe.stdout.trim() : null;
  return {
    name: 'msvc',
    label: 'MSVC v143',
    required: 'required',
    detected,
    status: detected ? 'ok' : 'fail',
    hint: detected ? '' : 'install "Desktop development with C++" workload from VS Build Tools 2022',
  };
}

export async function runChecks(platform = process.platform) {
  const results = [];
  for (const name of ['node', 'docker', 'powershell', 'git', 'rustc', 'cargo']) {
    results.push(await detectOne(name, platform));
  }
  results.push(await detectMsvc());
  return results;
}

function formatRequired(r) {
  if (typeof r === 'string') return r;
  return `>= ${r.major}.${r.minor}.${r.patch}`;
}

function formatDetected(r) {
  if (r.status === 'skip') return 'skipped';
  if (r.detected === null) return 'not found';
  if (typeof r.detected === 'string') return r.detected;
  return `${r.detected.major}.${r.detected.minor}.${r.detected.patch}`;
}

export function renderTable(results) {
  const headers = ['Tool', 'Required', 'Detected', 'Status'];
  const rows = results.map((r) => [r.label, formatRequired(r.required), formatDetected(r), r.status.toUpperCase()]);
  const widths = headers.map((h, i) => Math.max(h.length, ...rows.map((row) => row[i].length)));
  const sep = widths.map((w) => '-'.repeat(w)).join('   ');
  const lines = [];
  lines.push(headers.map((h, i) => h.padEnd(widths[i])).join('   '));
  lines.push(sep);
  for (const row of rows) lines.push(row.map((c, i) => c.padEnd(widths[i])).join('   '));
  return lines.join('\n');
}

if (isMain) {
  const sample = [
    { name: 'node', label: 'Node', required: { major: 18, minor: 0, patch: 0 }, detected: { major: 20, minor: 11, patch: 0 }, status: 'ok', hint: '' },
    { name: 'rust', label: 'Rust', required: { major: 1, minor: 75, patch: 0 }, detected: null, status: 'fail', hint: 'x' },
  ];
  const out = renderTable(sample);
  assert.match(out, /Node\s+>= 18\.0\.0\s+20\.11\.0\s+OK/);
  assert.match(out, /Rust\s+>= 1\.75\.0\s+not found\s+FAIL/);
  console.log('EnvChecker: renderTable assertions passed');

  const live = await runChecks();
  assert.equal(live.length, 7);
  for (const r of live) assert.ok(['ok', 'fail', 'skip'].includes(r.status), `bad status: ${r.status}`);
  console.log('EnvChecker: runChecks produced', live.length, 'results');
}
