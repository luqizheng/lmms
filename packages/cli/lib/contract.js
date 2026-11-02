'use strict';

// Contract layer: the one place that knows how a command is described and
// how an agent asks for its usage.
//
// Every command may declare a `spec`:
//   {
//     summary: 'one-line description',
//     usage: 'coder <cmd> [args]',
//     examples: ['coder <cmd> ...'],
//     flags: { '--version': { type: 'value' | 'flag', desc } },
//     positional: { name, required?, variadic?, desc? },
//     parse: 'custom',   // opt out of the generic validator
//   }
//
// From that single declaration we derive three things:
//   1. `--help` output (anywhere on the command line)
//   2. argument validation (unknown flag → USAGE)
//   3. the machine-readable schema exposed by `coder schema`

const { UsageError } = require('./errors.js');

function helpLines(name, spec) {
  const lines = [];
  lines.push(spec.usage ? `usage: ${spec.usage}` : `usage: coder ${name}`);
  if (spec.summary) lines.push('', spec.summary);

  const flags = spec.flags ?? {};
  const names = Object.keys(flags);
  if (names.length > 0) {
    const width = Math.max(...names.map((n) => n.length));
    lines.push('', 'flags:');
    for (const flag of names) {
      const declared = flags[flag];
      const normalized = declared === 'flag' || declared === 'value'
        ? { type: declared, desc: '' }
        : declared;
      const desc = normalized.desc ? normalized.desc : '';
      const metavar = normalized.type === 'value' ? ' <value>' : '';
      lines.push(`  ${(flag + metavar).padEnd(width + 8)}  ${desc}`.trimEnd());
    }
  }

  const positional = spec.positional;
  if (positional) {
    const shape = positional.variadic
      ? `<${positional.name}>...`
      : `<${positional.name}>`;
    const required = positional.required ? ' (required)' : '';
    lines.push('', `arguments:`, `  ${shape}${required}${positional.desc ? `  ${positional.desc}` : ''}`.trimEnd());
  }

  if (Array.isArray(spec.examples) && spec.examples.length > 0) {
    lines.push('', 'examples:');
    for (const example of spec.examples) lines.push(`  ${example}`);
  }

  lines.push('', 'exit codes: 0 ok · 1 failure · 2 usage · 3 guard · 4 external command');
  return lines;
}

function wantsHelp(argv) {
  return argv.some((token) => token === '--help' || token === '-h');
}

function schemaFor(name, spec) {
  const flags = spec.flags ?? {};
  const out = {
    name,
    summary: spec.summary ?? '',
    usage: spec.usage ?? `coder ${name}`,
    examples: spec.examples ?? [],
    flags: Object.keys(flags).map((flag) => {
      const declared = flags[flag];
      const normalized = declared === 'flag' || declared === 'value'
        ? { type: declared, desc: '' }
        : declared;
      return { name: flag, type: normalized.type, desc: normalized.desc ?? '' };
    }),
  };
  if (spec.positional) {
    out.positional = {
      name: spec.positional.name,
      required: Boolean(spec.positional.required),
      variadic: Boolean(spec.positional.variadic),
      desc: spec.positional.desc ?? '',
    };
  }
  if (spec.parse === 'custom') out.parse = 'custom';
  return out;
}

function assertKnownSpec(spec) {
  if (!spec) return;
  for (const [flag, declared] of Object.entries(spec.flags ?? {})) {
    const normalized = declared === 'flag' || declared === 'value' ? declared : declared?.type;
    if (normalized !== 'flag' && normalized !== 'value') {
      throw new Error(`spec for ${flag} must declare type 'flag' or 'value'`);
    }
  }
}

module.exports = { helpLines, wantsHelp, schemaFor, assertKnownSpec, UsageError };
