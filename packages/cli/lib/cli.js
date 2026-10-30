'use strict';

const path = require('node:path');
const { EXIT, CoderError, UsageError } = require('./errors.js');
const { createOutput } = require('./output.js');
const { COMMANDS } = require('./commands.js');
const { detectContext } = require('./context.js');

// Global flags apply only BEFORE the command name — after it, tokens belong
// to the command (e.g. `release bump --version X` must not be hijacked).
// Exception: --json is an output concern and is honored anywhere.
function splitGlobals(argv, baseCwd) {
  let json = false;
  let dir = baseCwd;
  let wantHelp = false;
  let wantVersion = false;
  let i = 0;
  while (i < argv.length) {
    const token = argv[i];
    if (token === '--json') { json = true; i++; }
    else if (token === '--cwd') { dir = path.resolve(baseCwd, argv[++i] ?? '.'); i++; }
    else if (token === '--help' || token === '-h') { wantHelp = true; i++; }
    else if (token === '--version') { wantVersion = true; i++; }
    else break;
  }
  const args = [];
  for (const token of argv.slice(i)) {
    if (token === '--json') { json = true; continue; }
    args.push(token);
  }
  return { args, json, dir, wantHelp, wantVersion };
}

function matchCommand(args) {
  let matched = null;
  for (const key of Object.keys(COMMANDS)) {
    const parts = key.split(' ');
    if (parts.length > args.length) continue;
    if (parts.every((p, i) => p === args[i])) {
      if (!matched || parts.length > matched.split(' ').length) matched = key;
    }
  }
  return matched;
}

function helpLines() {
  const keys = Object.keys(COMMANDS).sort();
  const width = Math.max(...keys.map((k) => k.length));
  return ['usage: coder [--json] [--cwd <dir>] <command> [args]', '', ...keys.map((k) => `  ${k.padEnd(width)}  ${COMMANDS[k].summary}`)];
}

function envelopeFor(err) {
  if (err instanceof CoderError) {
    return { exitCode: err.exitCode, data: null, warnings: [], errors: [{ code: err.code, message: err.message, details: err.details }] };
  }
  return { exitCode: EXIT.FAIL, data: null, warnings: [], errors: [{ code: 'FAIL', message: err && err.message ? err.message : String(err), details: {} }] };
}

async function run(argv = [], { cwd = process.cwd(), stdout = process.stdout, stderr = process.stderr } = {}) {
  const { args, json, dir, wantHelp, wantVersion } = splitGlobals(argv, cwd);
  const out = createOutput({ json, stdout, stderr });

  if (wantVersion) {
    stdout.write(require('../package.json').version + '\n');
    return EXIT.OK;
  }

  const finish = (command, result) => {
    out.emit({ command, result });
    return result.exitCode ?? EXIT.OK;
  };

  const key = matchCommand(args);
  if (!key) {
    if (wantVersion) {
      stdout.write(require('../package.json').version + '\n');
      return EXIT.OK;
    }
    if (wantHelp || args.length === 0) {
      return finish('help', { data: null, human: helpLines(), exitCode: args.length === 0 && !wantHelp ? EXIT.USAGE : EXIT.OK });
    }
    const err = new UsageError(`unknown command: ${args.join(' ') || '(none)'}`);
    const result = envelopeFor(err);
    if (!json) stderr.write(`✗ ${err.message}\n`);
    return finish('cli', result);
  }

  const context = detectContext(dir);
  const ctx = { root: context.root, context, out, json, cwd: dir };
  try {
    const rest = args.slice(key.split(' ').length);
    const result = await COMMANDS[key].run(rest, ctx);
    return finish(key, result);
  } catch (err) {
    const result = envelopeFor(err);
    if (!json) stderr.write(`✗ ${err.message}\n`);
    return finish(key, result);
  }
}

module.exports = { run };
