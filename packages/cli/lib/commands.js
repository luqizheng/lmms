'use strict';

// Command registry. Keys are full command names (space separated, e.g.
// 'release status'); values are { summary, run(args, ctx) -> Result }.
// Result = { data, human, exitCode?, warnings?, errors? }.
// From Task 3 on, command modules in commands/ export { commands } and are
// merged here; keep this file the single registry.
const COMMANDS = {
  version: {
    summary: 'print coder and node versions',
    run: async () => ({
      data: { node: process.version, cli: require('../package.json').version },
      human: [`coder ${require('../package.json').version}`, `node ${process.version}`],
    }),
  },
};

module.exports = { COMMANDS };
