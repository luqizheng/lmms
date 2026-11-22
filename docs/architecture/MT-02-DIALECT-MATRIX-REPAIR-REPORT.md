# MT-02 — Dialect Matrix and Migration Fixture Repair

**Status:** repository-side corrections committed; remote rerun intentionally not triggered  
**Branch:** `mt-02-dialect-ci-repair`

## Evidence reviewed

Inspected GitHub Actions run `44969202059` (commit `7a2f83b`):

- MySQL dialect job: passed.
- SQLite dialect job: passed.
- PostgreSQL dialect job: failed at Rust compilation because `pg_migration_test.rs` included two removed `config-service` migration paths.
- MySQL service job: the audit-service tests completed; the next matrix entry still invoked the retired `config_service` package, which is not a member of the current workspace.
- PostgreSQL service job: failed immediately while invoking feature-enabled tests against service packages. The workflow now uses package-qualified feature selectors to avoid ambiguous feature scoping.

## Changes

1. Removed the obsolete config-service PostgreSQL migration test and stale SQL includes.
2. Added the current file-service PostgreSQL V003 alignment migration to both the migration execution fixture and static migration checks.
3. Removed retired `config_service` invocations from MySQL and PostgreSQL service jobs.
4. Scoped service dialect features explicitly as `<package>/mysql` and `<package>/postgres`.
5. Corrected the workflow comment so it reflects actual workspace membership.
6. Corrected the Dameng feature to enable the crate's MySQL transport implementation, normalized `dameng://` to the `mysql://` scheme before passing the URL to SQLx, and aligned its current-time expression with Dameng's native `SYSDATE`. Added an opt-in real connectivity/ping test for CI.
7. Replaced the invalid MSSQL `--features mssql` database job (no SQL Server driver or Cargo feature exists) with MSSQL SQL-rendering contract tests. The workflow now explicitly does not claim SQL Server connectivity support.

## Verification status

- [x] Failure causes confirmed from GitHub Actions job logs.
- [x] Current workspace membership checked against the repository tree and Cargo manifests.
- [x] Updated migration fixture paths checked against the current repository tree.
- [x] Reviewed dialect implementation and aligned Dameng feature/URL handling with the MySQL-wire driver.
- [x] Removed the unsupported MSSQL connection-test claim; retained pure SQL rendering tests.
- [x] Changes committed to this branch.
- [ ] GitHub Actions rerun — intentionally not triggered while CI activation is paused.
- [ ] PostgreSQL migration execution against a live PostgreSQL server.
- [ ] Full service dialect matrix execution.

No build, integration test, or workflow rerun is claimed as passing after these edits.
