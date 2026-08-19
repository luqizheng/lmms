#!/usr/bin/env node
// scripts/perf-diff.js
// Compare k6 baseline JSON files (main branch artifacts vs PR candidate).
//
// Usage:
//   node scripts/perf-diff.js \
//     --base /tmp/perf-data/main-s2-login-20260819.json \
//     --pr   /tmp/perf-data/pr-s2-login-20260819.json \
//     [--tolerance 1.20]   # p95 regression threshold (default 1.20 = 20%)
//
// Exit code:
//   0  = all scenarios within tolerance
//   1  = at least one regression beyond tolerance
//   2  = error / invalid input
//
// Output: Markdown table to stdout.

'use strict';

const fs = require('node:fs');
const path = require('node:path');

function parseArgs(argv) {
  const out = { tolerance: 1.20 };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--base') out.base = argv[++i];
    else if (a === '--pr') out.pr = argv[++i];
    else if (a === '--tolerance') out.tolerance = Number(argv[++i]);
    else if (a === '--help' || a === '-h') {
      console.log(fs.readFileSync(__filename, 'utf8').split('\n').filter(l => l.startsWith('//')).slice(1).join('\n'));
      process.exit(0);
    }
  }
  return out;
}

function readSummary(p) {
  if (!fs.existsSync(p)) return null;
  return JSON.parse(fs.readFileSync(p, 'utf8'));
}

// Extract the http_req_duration percentile + http_req_failed rate for a k6 summary.
// k6 summary-export JSON shape:
//   { metrics: { http_req_duration: {p(90), p(95), ..., count, rate}, ... } }
// (not nested under .values; values is the in-memory Counter shape).
function extract(metrics) {
  if (!metrics || !metrics.http_req_duration) return null;
  const dur = metrics.http_req_duration;
  const reqs = metrics.http_reqs || {};
  const failed = metrics.http_req_failed || {};
  return {
    count: reqs.count || 0,
    rate: reqs.rate || 0,
    p50: dur.med || 0,
    p95: dur['p(95)'] || 0,
    p99: dur['p(99)'] || 0,
    errRate: failed.rate || 0,
  };
}

function pctDelta(base, pr) {
  if (!base || base === 0) return null;
  return ((pr - base) / base) * 100;
}

function verdict(base, pr, tolerance) {
  if (!base) return { ok: true, label: '—' };
  const checks = [
    { name: 'p95', delta: pctDelta(base.p95, pr.p95), limit: 20 },
    { name: 'p99', delta: pctDelta(base.p99, pr.p99), limit: 50 },
    { name: 'err', delta: (pr.errRate - base.errRate) * 100, limit: 5 },
  ];
  const regressions = checks.filter(c => c.delta !== null && c.delta > c.limit);
  if (regressions.length > 0) {
    return { ok: false, label: `❌ REGRESSION (${regressions.map(r => `${r.name}+${r.delta.toFixed(1)}%`).join(', ')})` };
  }
  const p95d = pctDelta(base.p95, pr.p95);
  if (p95d !== null && p95d < -10) {
    return { ok: true, label: '✅ improved' };
  }
  return { ok: true, label: '✅ within tolerance' };
}

function compareFileBasePath(basePath, prPath, tolerance) {
  const base = readSummary(basePath);
  const pr = readSummary(prPath);
  const baseMetrics = extract(base?.metrics);
  const prMetrics = extract(pr?.metrics);

  const scenario = path.basename(basePath).replace(/\.json$/, '');
  if (!baseMetrics || !prMetrics) {
    return {
      scenario,
      ok: true,
      base: null,
      pr: null,
      v: { ok: true, label: '— (missing file)' },
    };
  }

  const v = verdict(baseMetrics, prMetrics, tolerance);
  return { scenario, ok: v.ok, base: baseMetrics, pr: prMetrics, v };
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!args.base || !args.pr) {
    console.error('usage: node scripts/perf-diff.js --base <main.json> --pr <pr.json> [--tolerance 1.20]');
    process.exit(2);
  }

  // Allow --base to be a glob or comma-list; compare each pair (matched by basename).
  const baseFiles = args.base.includes(',')
    ? args.base.split(',')
    : (args.base.includes('*') ? require('node:fs').readdirSync(path.dirname(args.base)).filter(f => f.match(args.base.replace(/.*\//, ''))).map(f => path.join(path.dirname(args.base), f))
      : [args.base]);
  const prFiles = args.pr.includes(',')
    ? args.pr.split(',')
    : (args.pr.includes('*') ? require('node:fs').readdirSync(path.dirname(args.pr)).filter(f => f.match(args.pr.replace(/.*\//, ''))).map(f => path.join(path.dirname(args.pr), f))
      : [args.pr]);

  if (baseFiles.length !== prFiles.length) {
    console.error(`base count (${baseFiles.length}) != pr count (${prFiles.length})`);
    process.exit(2);
  }

  const rows = baseFiles.map((bp, i) => compareFileBasePath(bp, prFiles[i], args.tolerance));

  let md = '# k6 baseline diff\n\n';
  // Use fixed-point formatting to avoid floating-point artefacts like 19.999999999999996
  const tolPct = ((args.tolerance - 1) * 100).toFixed(0);
  md += `> tolerance: p95 ≤ +${tolPct}%, p99 ≤ +50%, error rate ≤ +5pp\n\n`;
  md += '| 场景 | 请求数 (base→pr) | p50 (ms) base→pr | p95 (ms) base→pr | p99 (ms) base→pr | 错误率 base→pr | 判定 |\n';
  md += '|------|------------------|------------------|------------------|------------------|----------------|------|\n';
  for (const r of rows) {
    if (!r.base || !r.pr) {
      md += `| ${r.scenario} | — | — | — | — | — | ${r.v.label} |\n`;
      continue;
    }
    md += `| ${r.scenario} | ${r.base.count}→${r.pr.count} | ${r.base.p50.toFixed(1)}→${r.pr.p50.toFixed(1)} | ${r.base.p95.toFixed(1)}→${r.pr.p95.toFixed(1)} | ${r.base.p99.toFixed(1)}→${r.pr.p99.toFixed(1)} | ${(r.base.errRate * 100).toFixed(2)}%→${(r.pr.errRate * 100).toFixed(2)}% | ${r.v.label} |\n`;
  }

  const regressions = rows.filter(r => !r.ok);
  if (regressions.length > 0) {
    md += `\n## Regressions detected\n\n`;
    for (const r of regressions) md += `- **${r.scenario}** — p95 ${r.base.p95.toFixed(1)}ms → ${r.pr.p95.toFixed(1)}ms\n`;
  }

  process.stdout.write(md + '\n');
  process.exit(regressions.length > 0 ? 1 : 0);
}

main();