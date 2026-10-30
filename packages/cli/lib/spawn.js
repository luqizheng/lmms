'use strict';

// Default exec factory shared by command modules. Human mode: child processes
// inherit our stdio (same UX as the retired scripts). JSON mode: child
// stdout/stderr are captured and re-emitted to OUR stderr, so process.stdout
// stays pure for the agent-parsed envelope (spec Review Focus #5).
// Caller opts pass through: cwd, env, shell (Windows .cmd shims), encoding.

const { spawnSync } = require('node:child_process');

function makeExec({ json = false } = {}) {
  return (file, args, opts = {}) => {
    const { stdio, ...rest } = opts;
    if (!json) {
      return spawnSync(file, args, { ...rest, stdio: stdio ?? 'inherit', shell: opts.shell ?? false });
    }
    const res = spawnSync(file, args, { ...rest, stdio: ['inherit', 'pipe', 'pipe'], shell: opts.shell ?? false });
    if (res.stdout) process.stderr.write(res.stdout);
    if (res.stderr) process.stderr.write(res.stderr);
    return res;
  };
}

module.exports = { makeExec };
