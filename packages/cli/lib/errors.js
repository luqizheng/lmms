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

class UsageError extends CoderError {
  constructor(message, details) { super('USAGE', message, details); this.exitCode = EXIT.USAGE; }
}

class GuardError extends CoderError {
  constructor(message, details) { super('GUARD', message, details); this.exitCode = EXIT.GUARD; }
}

class ExternalError extends CoderError {
  constructor(message, details) { super('EXTERNAL', message, details); this.exitCode = EXIT.EXTERNAL; }
}

module.exports = { EXIT, CoderError, UsageError, GuardError, ExternalError };
