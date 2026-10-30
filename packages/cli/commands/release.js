'use strict';

const { EXIT, UsageError, GuardError } = require('../lib/errors.js');
const { parseFlags } = require('../lib/args.js');
const { collectStatus } = require('../lib/release-status.js');
const { resolveTarget, bumpLedger, commitLedger } = require('../lib/release-bump.js');
const { normalizeVersion } = require('../lib/version.js');
const { publishMaven } = require('../lib/maven-publish.js');
const { publishRust } = require('../lib/rust-publish.js');
const repo = require('../lib/repo.js');
const fs = require('node:fs');

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

  'release bump': {
    summary: 'rewrite Makefile/CHANGELOG to the target version and commit (--dry-run to preview)',
    run: async (args, ctx) => {
      if (!ctx.root) throw new UsageError('release bump requires a repo context (--cwd)');
      const { flags } = parseFlags(args, { '--level': 'value', '--version': 'value', '--changelog': 'value', '--force': 'flag', '--from-tag': 'flag', '--dry-run': 'flag' });
      const changelogFile = flags['--changelog'];
      if (!changelogFile) throw new UsageError('--changelog <file> is required (body source, drafted from commit log)');
      const status = collectStatus({ root: ctx.root });
      const target = resolveTarget({
        level: flags['--level'] ?? 'minor',
        version: flags['--version'],
        force: Boolean(flags['--force']),
        fromTag: Boolean(flags['--from-tag']),
        status,
      });
      const body = fs.readFileSync(changelogFile, 'utf-8').trim();

      if (flags['--dry-run']) {
        const mkPath = 'release/dist-template/Makefile';
        const clPath = 'release/dist-template/CHANGELOG.md';
        const wouldChange = [];
        if (status.makefile.imageTag !== target.imageTag) wouldChange.push(mkPath);
        if (status.changelog.version !== target.imageTag) wouldChange.push(clPath);
        return {
          data: { target, dryRun: true, wouldChange, commit: false },
          human: [`target version: ${target.imageTag}${target.converge ? ' (converge to tag)' : ''}`, `would rewrite: ${wouldChange.join(', ') || '(nothing)'}`, 'dry run — nothing written, nothing committed'],
        };
      }

      const { changed } = bumpLedger({ root: ctx.root, imageTag: target.imageTag, changelogBody: body });
      const { commit } = commitLedger({ root: ctx.root, imageTag: target.imageTag });
      return {
        data: { target, changed, commit },
        human: [`ledger rewritten: ${changed.join(', ')}`, `commit: ${commit}`, 'next: coder release tag'],
        warnings: ['ledger commit created — pushing is a separate, human-confirmed step'],
      };
    },
  },

  'release tag': {
    summary: 'create local annotated tag vX.Y.Z (never pushes; dry-run supported)',
    run: async (args, ctx) => {
      if (!ctx.root) throw new UsageError('release tag requires a repo context (--cwd)');
      const { flags, positional } = parseFlags(args, { '--dry-run': 'flag' });
      const dryRun = Boolean(flags['--dry-run']);

      const dirty = repo.dirtyCount(ctx.root);
      if (dirty !== 0) throw new GuardError('DIRTY_WORKTREE', `worktree is not clean (${dirty} pending change(s)); commit or stash first`, { dirty });

      const status = collectStatus({ root: ctx.root });
      if (!status.consistent) throw new GuardError('VERSION_DRIFT', `version ledger drifted: ${status.drift.join(', ')}`, { drift: status.drift });

      const imageTag = positional[0] ?? status.git.imageTag;
      const parsed = normalizeVersion(imageTag);
      if (!parsed) throw new GuardError('NO_BASELINE', 'no version to tag (and none given)', { imageTag });
      const tag = parsed.tag;

      const exists = repo.git(ctx.root, ['rev-parse', '-q', '--verify', `refs/tags/${tag}`]);
      if (exists) throw new GuardError('TAG_EXISTS', `tag ${tag} already exists`, { tag });

      const next = `git push origin main ${tag}`;
      if (dryRun) {
        return { data: { tag, dryRun: true, wouldCreate: true, pushed: false, next }, human: [`would create ${tag}`, `after confirmation: ${next}`] };
      }
      repo.git(ctx.root, ['tag', '-a', tag, '-m', tag]);
      return {
        data: { tag, pushed: false, next },
        human: [`created ${tag} (local only, NOT pushed)`, 'after human confirmation run:', `  ${next}`],
        warnings: ['pushing the tag triggers the four-way publish pipeline — explicit confirmation required'],
      };
    },
  },

  'release publish maven': {
    summary: 'publish Java SDK to Reposilite (--kind release|snapshot, curl mode default)',
    run: async (args, ctx) => {
      if (!ctx.root) throw new UsageError('release publish maven requires a repo context (--cwd)');
      const { flags } = parseFlags(args, {
        '--version': 'value', '--kind': 'value', '--profile': 'value', '--modules': 'value',
        '--user': 'value', '--pass': 'value', '--repo-id': 'value', '--repo-url': 'value',
        '--mvn': 'flag', '--dry-run': 'flag',
      });
      const result = publishMaven({
        root: ctx.root,
        version: flags['--version'],
        kind: flags['--kind'] ?? 'release',
        profile: flags['--profile'] ?? 'java8',
        modules: flags['--modules'] ?? '',
        user: flags['--user'] ?? '',
        pass: flags['--pass'] ?? '',
        repoId: flags['--repo-id'] ?? '',
        repoUrl: flags['--repo-url'] ?? '',
        useMvn: Boolean(flags['--mvn']),
        dryRun: Boolean(flags['--dry-run']),
        out: ctx.out,
      });
      const human = result.dryRun
        ? ['dry-run OK: pom bump planned, mvn clean verify passed, nothing deployed']
        : [`deployed ${result.uploaded}/${result.modules} artifacts to ${result.repoId} @ ${result.repoUrl}`, 'pom.xml version change is NOT auto-committed: git add sdk/java && git commit -m "feat(sdk): bump"'];
      return { data: result, human, warnings: result.dryRun ? [] : ['pom.xml change requires a follow-up commit (see above)'] };
    },
  },

  'release publish rust': {
    summary: 'publish libs/*(17) + sdk/rust/*(7) crates to the registry (--targets all|libs|sdks)',
    run: async (args, ctx) => {
      if (!ctx.root) throw new UsageError('release publish rust requires a repo context (--cwd)');
      const { flags, positional } = parseFlags(args, { '--dry-run': 'flag', '--registry': 'value', '--targets': 'value' });
      const version = positional[0];
      if (!version && !flags['--dry-run']) throw new UsageError('RUST_VERSION_REQUIRED', 'a version positional is required (e.g. coder release publish rust 1.7.0)');
      const result = publishRust({
        root: ctx.root,
        version: version ?? '',
        registry: flags['--registry'] ?? 'kellnr',
        targets: flags['--targets'] ?? 'all',
        dryRun: Boolean(flags['--dry-run']),
        out: ctx.out,
      });
      const human = result.dryRun
        ? [`dry-run plan: ${result.plan.length} crates (nothing executed)`]
        : [`published ${result.published.length} crates to ${flags['--registry'] ?? 'kellnr'}`, 'Cargo.toml path deps restored to originals'];
      return { data: result, human };
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
