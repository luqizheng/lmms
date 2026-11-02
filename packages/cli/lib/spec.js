'use strict';

// Declarative command spec parser.
//
// Every command declares its flags up front; this module turns argv into
// { flags, positional } and rejects anything the spec does not declare.
// Rationale: an agent that typos a flag must get an error, never a silent
// pass (the pre-B0 behavior swallowed unknown flags into `positional`).
//
// Spec shape:
//   {
//     flags: { '--version': { type: 'value' | 'flag', desc } },
//     positional: { name, required?, variadic?, desc? },
//   }
//
// `--` terminates flag parsing: everything after it is positional.

const { UsageError } = require('./errors.js');

function flagSpec(spec, name) {
  const flags = spec.flags ?? {};
  const declared = flags[name];
  if (!declared) return null;
  if (declared === 'flag' || declared === 'value') return { type: declared };
  return { type: declared.type ?? 'flag', desc: declared.desc };
}

function parseSpec(spec, argv) {
  const flags = {};
  const positional = [];
  const positionalSpec = spec.positional ?? null;
  let flagsEnded = false;

  for (let i = 0; i < argv.length; i++) {
    const token = argv[i];

    if (flagsEnded) {
      positional.push(token);
      continue;
    }
    if (token === '--') {
      flagsEnded = true;
      continue;
    }

    // Only `--long` tokens are spec-validated. Short options (-v, -p) and
    // bare words are command-specific; commands with custom grammars opt out
    // via `parse: 'custom'` and never reach this module.
    if (token.startsWith('--') && token.length > 2) {
      const declared = flagSpec(spec, token);
      if (!declared) {
        throw new UsageError('UNKNOWN_FLAG', `unknown flag: ${token}`, { flag: token });
      }
      if (declared.type === 'value') {
        const value = argv[i + 1];
        if (value === undefined || value.startsWith('--')) {
          throw new UsageError('MISSING_FLAG_VALUE', `${token} requires a value`, { flag: token });
        }
        flags[token] = value;
        i++;
      } else {
        flags[token] = true;
      }
      continue;
    }

    positional.push(token);
  }

  if (!positionalSpec) {
    if (positional.length > 0) {
      throw new UsageError(
        'TOO_MANY_POSITIONAL',
        `unexpected argument: ${positional[0]}`,
        { unexpected: positional },
      );
    }
  } else if (positionalSpec.variadic) {
    // any count accepted
  } else if (positionalSpec.required && positional.length === 0) {
    throw new UsageError(
      'MISSING_POSITIONAL',
      `missing required argument <${positionalSpec.name}>`,
      { name: positionalSpec.name },
    );
  } else if (!positionalSpec.variadic && positional.length > 1) {
    throw new UsageError(
      'TOO_MANY_POSITIONAL',
      `too many arguments (expected at most 1 <${positionalSpec.name}>, got ${positional.length})`,
      { expected: 1, got: positional.length },
    );
  }

  return { flags, positional };
}

module.exports = { parseSpec };
