'use strict';

const { EXIT } = require('../lib/errors.js');
const { collectStatus } = require('../lib/release-status.js');

const commands = {
  'release status': {
    summary: 'reconcile git tag / Makefile IMAGE_TAG / CHANGELOG versions',
    run: async (args, ctx) => {
      const status = collectStatus({ root: ctx.root });
      const warnings = [];
      if (status.git.latestTag == null) {
        warnings.push('no v* tag found (or git unavailable) — cannot establish the authoritative version');
      }
      if (!status.consistent) {
        warnings.push('ledger drift: git tag is authoritative — run `coder release bump --force --from-tag` to converge');
      }
      const human = [
        `git tag    ${status.git.latestTag ?? '(none)'}`,
        `Makefile   ${status.makefile.imageTag ?? '(missing)'}`,
        `CHANGELOG  ${status.changelog.version ?? '(missing)'}`,
        status.consistent ? 'ledgers consistent' : `drift: ${status.drift.join(', ')}`,
      ];
      return {
        data: status,
        human,
        warnings,
        exitCode: status.consistent ? EXIT.OK : EXIT.GUARD,
      };
    },
  },

  doctor: {
    summary: 'show detected context and available command surface',
    run: async (args, ctx) => {
      const byContext = {
        monorepo: ['release  status / bump / tag / publish (maven|rust|npm)', 'build    images', 'assets   download'],
        dist: ['ops  secrets / up / verify / upgrade (batch 2)'],
        unknown: ['context not detected — run inside the monorepo or a dist package'],
      };
      const surface = byContext[ctx.context.kind] ?? byContext.unknown;
      const data = { context: ctx.context.kind, root: ctx.root, node: process.version, status: 'ok' };
      const human = [`context: ${ctx.context.kind}`, `root: ${ctx.root ?? '(none)'}`, `node: ${process.version}`, '', ...surface];
      return { data, human };
    },
  },
};

module.exports = { commands };
