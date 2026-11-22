# MT-02 — Database Dialect Matrix and Migration Fixture Repair

**Status:** implementation updated; GitHub Actions validation is pending on the PR head.  
**Branch:** `mt-02-dialect-ci-repair`

## Evidence and original CI failures

The original matrix exposed stale migration fixtures for the retired config-service package and service test commands that did not select package features correctly. Those failures are addressed by the current workflow and migration fixture updates.

## Scope decision

The repository owner confirmed that Dameng and MSSQL are not required:
- Rust/SQLx has no supported Dameng driver in this project; do not emulate it through the MySQL wire protocol.
- There are no current MSSQL customers; defer SQL Server support until a concrete future requirement exists.

Accordingly, MT-02 removes both from the supported product surface rather than maintaining non-functional placeholders.

## Changes

1. Removed the Dameng Cargo feature, URL normalization shim, connection test, dialect implementation, and dialect assertions.
2. Removed the MSSQL dialect implementation and SQL-rendering-only claims/tests.
3. Reduced `DialectKind` to MySQL, PostgreSQL, and SQLite; removed Dameng/MSSQL branches from pool initialization and migration routing. Unknown URL schemes now return `None` during dialect detection and fail pool initialization with `DB_URL_SCHEME` instead of silently defaulting to MySQL.
7. Added dialect tests proving Dameng, MSSQL, Oracle, and empty URLs are rejected while all supported URL schemes are recognized.
4. Removed Dameng/MSSQL CI jobs and the now-unneeded scheduled workflow trigger.
5. Updated the dialect support specification to list only the three supported databases.
6. Retained the earlier migration fixture and service test matrix corrections.

## Verification

- [x] Repository code and manifests updated to remove explicit Dameng/MSSQL implementation hooks.
- [x] CI workflow updated to test only supported database backends.
- [ ] GitHub Actions run for the latest PR head.
- [ ] PostgreSQL migration execution against a live PostgreSQL server.

No test is claimed as passing until the latest GitHub Actions run reports success.