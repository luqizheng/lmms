'use strict';

// Unified output layer. Human mode: logs and `human` lines go to stdout.
// JSON mode: exactly one envelope line on stdout, everything else to stderr —
// agents parse stdout, so it must stay pure (spec Review Focus #5).
function createOutput({ json = false, stdout = process.stdout, stderr = process.stderr } = {}) {
  const writeLog = (msg) => { (json ? stderr : stdout).write(String(msg) + '\n'); };
  return {
    json,
    log: writeLog,
    warn: writeLog,
    error: writeLog,
    emit({ command, result }) {
      if (json) {
        const exitCode = result.exitCode ?? 0;
        const envelope = {
          command,
          ok: exitCode === 0,
          exitCode,
          data: result.data ?? null,
          warnings: result.warnings ?? [],
          errors: result.errors ?? [],
        };
        stdout.write(JSON.stringify(envelope) + '\n');
        return;
      }
      const { human } = result;
      if (Array.isArray(human)) human.forEach((line) => stdout.write(String(line) + '\n'));
      else if (human != null) stdout.write(String(human) + '\n');
    },
  };
}

module.exports = { createOutput };
