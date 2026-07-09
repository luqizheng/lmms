#!/usr/bin/env node
// scripts/bench.js
// 单 service 压测（HTTP GET /health）。接口与输出严格按
// docs/superpowers/specs/phase4-perf-circuit-limit.md 约定。
//
// 用法：
//   node scripts/bench.js <service> [duration=60] [concurrency=10]
//   BENCH_URL=http://localhost:8080 BENCH_PATH=/health node scripts/bench.js audit-service 30 10
//
// 默认打 http://localhost:8080/health（与 spec 输出格式一致）。service 参数仅作为输出 label。

'use strict';

const http = require('node:http');

const DEFAULT_URL = process.env.BENCH_URL || 'http://localhost:8080';
const DEFAULT_PATH = process.env.BENCH_PATH || '/health';
const REQUEST_TIMEOUT_MS = 5000;

function pickRequest() {
  return http;
}

function makeRequest(url, timeoutMs) {
  return new Promise((resolve) => {
    const start = process.hrtime.bigint();
    const lib = pickRequest();
    const req = lib.get(url, { timeout: timeoutMs }, (res) => {
      const end = process.hrtime.bigint();
      const latencyMs = Number(end - start) / 1e6;
      res.resume();
      res.on('end', () => {
        resolve({
          ok: res.statusCode >= 200 && res.statusCode < 400,
          status: res.statusCode,
          latencyMs,
        });
      });
    });
    req.on('error', () => {
      const end = process.hrtime.bigint();
      resolve({
        ok: false,
        status: 0,
        latencyMs: Number(end - start) / 1e6,
        error: true,
      });
    });
    req.on('timeout', () => {
      req.destroy();
      resolve({ ok: false, status: 0, latencyMs: timeoutMs, timeout: true });
    });
  });
}

async function loadTest(url, durationSec, concurrency) {
  const deadline = Date.now() + durationSec * 1000;
  const inflight = new Set();
  const latencies = [];
  let total = 0, success = 0, errors = 0, timeouts = 0;

  process.stdout.write(`Starting load test:\n  URL: ${url}\n  Duration: ${durationSec}s\n  Concurrency: ${concurrency}\n\n`);

  for (let i = 0; i < concurrency; i++) {
    const p = (async () => {
      while (Date.now() < deadline) {
        const r = await makeRequest(url, REQUEST_TIMEOUT_MS);
        total++;
        latencies.push(r.latencyMs);
        if (r.timeout) timeouts++;
        else if (r.ok) success++;
        else errors++;
      }
    })();
    inflight.add(p);
    p.finally(() => inflight.delete(p));
  }
  await Promise.all(inflight);

  latencies.sort((a, b) => a - b);
  const sum = latencies.reduce((s, v) => s + v, 0);
  const rps = total / durationSec;
  const pct = (q) => latencies[Math.min(latencies.length - 1, Math.floor(latencies.length * q))] || 0;

  console.log('=== Load Test Results ===');
  console.log(`Duration:    ${durationSec.toFixed(2)}s`);
  console.log(`Total Req:   ${total}`);
  console.log(`RPS:         ${rps.toFixed(2)}`);
  console.log('');
  console.log('Responses:');
  console.log(`  Success:   ${success}`);
  console.log(`  Errors:    ${errors}`);
  console.log(`  Timeouts:  ${timeouts}`);
  console.log('');
  console.log('Latency (ms):');
  console.log(`  Avg:       ${latencies.length ? (sum / latencies.length).toFixed(2) : '0.00'}`);
  console.log(`  P50:       ${pct(0.5).toFixed(2)}`);
  console.log(`  P90:       ${pct(0.9).toFixed(2)}`);
  console.log(`  P99:       ${pct(0.99).toFixed(2)}`);
  console.log(`  Max:       ${(latencies[latencies.length - 1] || 0).toFixed(2)}`);
}

function parseArgs(argv) {
  const service = argv[0];
  if (!service) return null;
  const duration = Number(argv[1]) || 60;
  const concurrency = Number(argv[2]) || 10;
  return { service, duration, concurrency };
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!args) {
    console.error('usage: node scripts/bench.js <service> [duration] [concurrency]');
    console.error('  service:     label only (e.g. audit-service)');
    console.error('  duration:    seconds (default 60)');
    console.error('  concurrency: parallel workers (default 10)');
    process.exit(2);
  }

  const url = `${DEFAULT_URL}${DEFAULT_PATH}`;
  console.log(`[bench] target service label: ${args.service}`);
  loadTest(url, args.duration, args.concurrency).catch((err) => {
    console.error(`[bench] failed: ${err.message}`);
    process.exit(1);
  });
}

main();