'use strict';

/**
 * lib/cli.js — tiny argv parser.
 *
 * Supported flags:
 *   --dry-run            Plan only, no file modifications
 *   --check              Print current sync state, then exit
 *   --help / -h          Print help
 *   --root <path>        Project root (default: cwd)
 *   --policy <path>      Override policy file location
 *   --lock <path>        Override lock file location
 *
 * Positional: first non-flag arg is the source (tarball path or URL).
 */

const EXIT_OK = 0;
const EXIT_USAGE = 64;      // EX_USAGE
const EXIT_ERROR = 1;
const EXIT_CONFLICT = 2;

function parseArgs(argv) {
  const opts = {
    dryRun: false,
    check: false,
    help: false,
    root: process.cwd(),
    policy: null,
    lock: null,
    source: null,
  };

  const positional = [];

  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];

    if (a === '--dry-run') {
      opts.dryRun = true;
    } else if (a === '--check') {
      opts.check = true;
    } else if (a === '--help' || a === '-h') {
      opts.help = true;
    } else if (a === '--root') {
      opts.root = argv[++i];
    } else if (a.startsWith('--root=')) {
      opts.root = a.slice('--root='.length);
    } else if (a === '--policy') {
      opts.policy = argv[++i];
    } else if (a.startsWith('--policy=')) {
      opts.policy = a.slice('--policy='.length);
    } else if (a === '--lock') {
      opts.lock = argv[++i];
    } else if (a.startsWith('--lock=')) {
      opts.lock = a.slice('--lock='.length);
    } else if (a.startsWith('-')) {
      throw new Error(`unknown flag: ${a}`);
    } else {
      positional.push(a);
    }
  }

  if (positional.length > 1) {
    throw new Error(`multiple positional args: ${positional.join(', ')}`);
  }
  opts.source = positional[0] || null;

  return opts;
}

module.exports = {
  parseArgs,
  EXIT_OK,
  EXIT_USAGE,
  EXIT_ERROR,
  EXIT_CONFLICT,
};