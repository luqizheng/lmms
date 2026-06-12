#!/usr/bin/env node
// test-orchestrator.js
// 入口：Docker 启动、DB 重置、测试运行、报告生成、AI 提示词
//
// 用法：
//   node scripts/test-orchestrator.js start
//   node scripts/test-orchestrator.js stop
//   node scripts/test-orchestrator.js reset-db
//   node scripts/test-orchestrator.js test-unit [--service <name>]
//   node scripts/test-orchestrator.js test-integration [--service <name>]
//   node scripts/test-orchestrator.js test-all [--service <name>]
//   node scripts/test-orchestrator.js report
//   node scripts/test-orchestrator.js fix-prompt
//   node scripts/test-orchestrator.js generate-prompt
//   node scripts/test-orchestrator.js validate-specs
//   node scripts/test-orchestrator.js loop

const fs = require('node:fs');
const path = require('node:path');
const { ROOT, log, runDir: getRunDir } = require('./lib/utils.js');
const docker = require('./lib/Docker-Helper.js');
const { runServiceTests, getServiceList } = require('./lib/Test-Runner.js');
const { buildSummary, buildFixPrompt } = require('./lib/Report-Builder.js');
const { renderSpecToCodePrompt } = require('./lib/Prompt-Renderer.js');
const { invokeSpecValidation } = require('./lib/Spec-Validator.js');

function parseArgs(argv) {
  const out = { command: null, service: null, timestamp: null };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--service' || a === '-s') out.service = argv[++i];
    else if (a === '--timestamp' || a === '-t') out.timestamp = argv[++i];
    else if (!out.command) out.command = a;
  }
  return out;
}

function initializeRunDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

async function runTestPhase(type, svc) {
  const dir = initializeRunDir(getRunDir());
  const services = svc ? [svc] : getServiceList();
  for (const s of services) {
    const json = path.join(dir, `${s}.${type}.json`);
    log.cyan(`[test] ${s} (${type}) -> ${json}`);
    try {
      await runServiceTests(s, type, json);
    } catch (e) {
      log.red(`[test] ${s} (${type}) FAILED to run: ${e.message}`);
    }
  }
  return dir;
}

function resolveRunDir(timestamp) {
  if (timestamp) return getRunDir(timestamp);
  const dir = path.join(ROOT, 'reports');
  if (!fs.existsSync(dir)) return null;
  const entries = fs.readdirSync(dir)
    .filter((n) => n.startsWith('run-'))
    .sort()
    .reverse();
  if (!entries.length) return null;
  return path.join(dir, entries[0]);
}

async function cmdReport(timestamp) {
  const dir = resolveRunDir(timestamp);
  if (!dir || !fs.existsSync(dir)) throw new Error('no run dir (specify --timestamp or run test-all first)');
  const result = buildSummary(dir);
  log.green(`[report] ${result.SummaryPath}`);
  if (result.Failing.length > 0) {
    const fix = buildFixPrompt(dir, result.Failing);
    log.yellow(`[report] failures detected, fix-prompt: ${fix}`);
    log.yellow('[report] latest: reports\\latest-fix-prompt.md');
  }
}

async function cmdFixPrompt(timestamp) {
  const dir = resolveRunDir(timestamp);
  if (!dir || !fs.existsSync(dir)) throw new Error('no run dir (specify --timestamp or run test-all first)');
  const result = buildSummary(dir);
  if (result.Failing.length === 0) {
    log.green('[fix-prompt] no failures in latest run');
  } else {
    const fix = buildFixPrompt(dir, result.Failing);
    log.yellow(`[fix-prompt] ${fix}`);
  }
}

async function cmdLoop(service) {
  await docker.startTestStack();
  await docker.resetDatabase();
  const dir = await runTestPhase('unit', service);
  await runTestPhase('integration', service);
  const result = buildSummary(dir);
  log.green(`[report] ${result.SummaryPath}`);
  if (result.Failing.length > 0) {
    const fix = buildFixPrompt(dir, result.Failing);
    log.yellow('[loop] failures detected.');
    log.yellow(`[loop] open: ${fix}`);
    log.yellow('[loop] also: reports\\latest-fix-prompt.md');
    log.yellow('[loop] (paste to AI; after AI edits, re-run: node scripts/test-orchestrator.js reset-db && node scripts/test-orchestrator.js test-integration)');
  } else {
    log.green('[loop] all tests passed');
  }
}

async function main() {
  const { command, service, timestamp } = parseArgs(process.argv);
  if (!command) {
    log.red('usage: node scripts/test-orchestrator.js <command> [options]');
    log.plain('commands: start, stop, reset-db, test-unit, test-integration, test-all, report, fix-prompt, generate-prompt, validate-specs, loop');
    process.exit(1);
  }

  try {
    switch (command) {
      case 'start':            await docker.startTestStack(); break;
      case 'stop':             await docker.stopTestStack(); break;
      case 'reset-db':         await docker.resetDatabase(); break;
      case 'test-unit':        await runTestPhase('unit', service); break;
      case 'test-integration': await runTestPhase('integration', service); break;
      case 'test-all': {
        const dir = await runTestPhase('unit', service);
        await runTestPhase('integration', service);
        log.green(`[done] logs: ${dir}`);
        break;
      }
      case 'report':           await cmdReport(timestamp); break;
      case 'fix-prompt':       await cmdFixPrompt(timestamp); break;
      case 'generate-prompt':  renderSpecToCodePrompt(); break;
      case 'validate-specs':   invokeSpecValidation(); break;
      case 'loop':             await cmdLoop(service); break;
      default:
        log.red(`unknown command: ${command}`);
        log.plain('valid: start, stop, reset-db, test-unit, test-integration, test-all, report, fix-prompt, generate-prompt, validate-specs, loop');
        process.exit(1);
    }
  } catch (e) {
    log.red(`error: ${e.message}`);
    process.exit(1);
  }
}

main();
