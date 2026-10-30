'use strict';

// Command registry. Keys are full command names (space separated, e.g.
// 'release status'); values are { summary, run(args, ctx) -> Result }.
// Result = { data, human, exitCode?, warnings?, errors? }.
// Command modules in commands/ export { commands } and are merged here —
// this file stays the single registry.
const COMMANDS = {
  version: {
    summary: 'print coder and node versions',
    run: async () => ({
      data: { node: process.version, cli: require('../package.json').version },
      human: [`coder ${require('../package.json').version}`, `node ${process.version}`],
    }),
  },
};

Object.assign(COMMANDS, require('../commands/release.js').commands);
Object.assign(COMMANDS, require('../commands/assets.js').commands);
Object.assign(COMMANDS, require('../commands/build.js').commands);

module.exports = { COMMANDS };
