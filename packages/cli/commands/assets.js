'use strict';

const { UsageError } = require('../lib/errors.js');
const { parseFlags } = require('../lib/args.js');
const { ensureAsset } = require('../lib/assets.js');

const commands = {
  'assets download': {
    summary: 'ensure / check (--check) / force (--force) the vendored swagger-ui zip',
    run: async (args, ctx) => {
      if (!ctx.root) throw new UsageError('assets download requires a repo context (--cwd)');
      const { flags } = parseFlags(args, { '--check': 'flag', '--force': 'flag' });
      const mode = flags['--check'] ? 'check' : flags['--force'] ? 'force' : 'ensure';
      const result = await ensureAsset({ root: ctx.root, mode });
      const human = mode === 'check'
        ? (result.existed ? `asset ok: ${result.path}` : `asset missing: ${result.path}`)
        : (result.downloaded
          ? [`downloaded ${result.path}`, 'sha256 verified']
          : `already present: ${result.path}`);
      return { data: { mode, ...result }, human };
    },
  },
};

module.exports = { commands };
