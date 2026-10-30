'use strict';

// Shared argv parsing for command modules: specs map a token to 'flag'
// (boolean) or 'value' (consumes the next token); anything unlisted is
// positional.
const { UsageError } = require('./errors.js');

function parseFlags(args, specs) {
  const flags = {};
  const positional = [];
  for (let i = 0; i < args.length; i++) {
    const token = args[i];
    const spec = specs[token];
    if (spec === 'value') {
      const value = args[++i];
      if (value == null) throw new UsageError(`${token} requires a value`);
      flags[token] = value;
    } else if (spec === 'flag') {
      flags[token] = true;
    } else {
      positional.push(token);
    }
  }
  return { flags, positional };
}

module.exports = { parseFlags };
