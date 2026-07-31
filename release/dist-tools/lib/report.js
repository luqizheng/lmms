'use strict';

/**
 * lib/report.js — colored terminal output.
 *
 * Behavior:
 *   - Colors only when stdout is a TTY (no escape codes in pipes/logs).
 *   - Honors NO_COLOR env var per https://no-color.org.
 *   - Single point of truth for log levels / labels.
 */

const NO_COLOR = 'NO_COLOR' in process.env;

function useColor() {
  return !NO_COLOR && process.stdout.isTTY;
}

const COLORS = {
  red:    useColor() ? '\x1b[0;31m' : '',
  green:  useColor() ? '\x1b[0;32m' : '',
  yellow: useColor() ? '\x1b[1;33m' : '',
  blue:   useColor() ? '\x1b[0;34m' : '',
  dim:    useColor() ? '\x1b[2m'    : '',
  off:    useColor() ? '\x1b[0m'    : '',
};

function fmt(color, text) {
  return `${COLORS[color]}${text}${COLORS.off}`;
}

function log(msg)   { console.log(`${fmt('blue', '[cms-sync]')} ${msg}`); }
function ok(msg)    { console.log(`${fmt('blue', '[cms-sync]')} ${fmt('green', '✓')} ${msg}`); }
function warn(msg)  { console.log(`${fmt('blue', '[cms-sync]')} ${fmt('yellow', '⚠')} ${msg}`); }
function err(msg)   { console.error(`${fmt('blue', '[cms-sync]')} ${fmt('red', '✗')} ${msg}`); }
function dim(msg)   { console.log(`${fmt('dim', msg)}`); }

function rule() {
  console.log(fmt('dim', '─'.repeat(70)));
}

function section(title) {
  console.log('');
  rule();
  console.log(`${fmt('blue', '[cms-sync]')} ${title}`);
  rule();
}

function item(symbol, color, label) {
  console.log(`  ${fmt(color, symbol)} ${fmt(color, label)}`);
}

const SYMBOLS = {
  updated:   { symbol: 'updated',   color: 'green' },
  merged:    { symbol: 'merged',    color: 'green' },
  unchanged: { symbol: 'unchanged', color: 'dim'    },
  conflict:  { symbol: 'CONFLICT',  color: 'red'   },
  newfile:   { symbol: 'new file',  color: 'green' },
  skipped:   { symbol: 'skipped',   color: 'yellow'},
};

module.exports = {
  log, ok, warn, err, dim, rule, section, item, SYMBOLS,
};