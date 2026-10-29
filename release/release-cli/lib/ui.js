'use strict';

/**
 * lib/ui.js — minimal ANSI styling, zero dependencies.
 *
 * Kept standalone (instead of reusing scripts/lib/ui.js) so the published
 * package stays dependency-free, matching the @digua/dist-tools approach.
 */

const ENABLED =
  process.stdout.isTTY &&
  process.env.NO_COLOR !== '1' &&
  process.env.TERM !== 'dumb';

function wrap(code, text) {
  return ENABLED ? `\x1b[${code}m${text}\x1b[0m` : String(text);
}

const chalk = {
  green: (t) => wrap(32, t),
  red: (t) => wrap(31, t),
  yellow: (t) => wrap(33, t),
  cyan: (t) => wrap(36, t),
  dim: (t) => wrap(2, t),
  bold: (t) => wrap(1, t),
};

const OK = chalk.green('\u2713');
const FAIL = chalk.red('\u2717');
const WARN = chalk.yellow('!');

module.exports = { chalk, OK, FAIL, WARN };
