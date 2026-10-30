'use strict';

// Exit-code contract (spec: docs/superpowers/specs/2026-10-30-coder-cli-engineering-shell-design.md):
// 0 ok / 1 unclassified failure / 2 usage / 3 unmet precondition / 4 external command failed.
const EXIT = { OK: 0, FAIL: 1, USAGE: 2, GUARD: 3, EXTERNAL: 4 };

class CoderError extends Error {
  constructor(code, message, details = {}) {
    super(message);
    this.name = this.constructor.name;
    this.code = code;
    this.details = details;
    this.exitCode = EXIT.FAIL;
  }
}

// Subclass constructors accept either (message, details) — defaulting to the
// class code — or (semanticCode, message, details) so guards can carry a
// machine-readable code like 'VERSION_DRIFT' in the JSON envelope.
class UsageError extends CoderError {
  constructor(a, b, c) {
    if (typeof b === 'string') super(a, b, c);
    else super('USAGE', a, b);
    this.exitCode = EXIT.USAGE;
  }
}

class GuardError extends CoderError {
  constructor(a, b, c) {
    if (typeof b === 'string') super(a, b, c);
    else super('GUARD', a, b);
    this.exitCode = EXIT.GUARD;
  }
}

class ExternalError extends CoderError {
  constructor(a, b, c) {
    if (typeof b === 'string') super(a, b, c);
    else super('EXTERNAL', a, b);
    this.exitCode = EXIT.EXTERNAL;
  }
}

module.exports = { EXIT, CoderError, UsageError, GuardError, ExternalError };
