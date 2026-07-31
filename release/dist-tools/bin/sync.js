#!/usr/bin/env node
'use strict';

/**
 * bin/sync.js — CLI entry for `cms-sync`.
 *
 * Dispatches:
 *   --check                       Show current sync state
 *   --help / -h                   Print this help
 *   <source>                      Sync from a tarball path or URL
 *   --dry-run <source>            Plan without modifying files
 */

const { runSync } = require('../lib/sync.js');
const { runCheck } = require('../lib/check.js');
const { printHelp } = require('../lib/help.js');
const { parseArgs, EXIT_OK, EXIT_USAGE, EXIT_ERROR } = require('../lib/cli.js');

async function main() {
  const opts = parseArgs(process.argv.slice(2));

  if (opts.help) {
    printHelp();
    process.exit(EXIT_OK);
  }

  if (opts.check) {
    const code = await runCheck(opts);
    process.exit(code);
  }

  if (!opts.source) {
    console.error('[cms-sync] ✗ missing source: provide a tarball path or URL');
    printHelp();
    process.exit(EXIT_USAGE);
  }

  try {
    const code = await runSync(opts);
    process.exit(code);
  } catch (err) {
    console.error(`[cms-sync] ✗ ${err.message}`);
    if (process.env.CMS_SYNC_DEBUG) console.error(err.stack);
    process.exit(EXIT_ERROR);
  }
}

main();