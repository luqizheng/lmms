# token-service JWT Key Lifecycle Operations Hardening — Design

Date: 2026-11-23
Status: implemented
Scope: services/token-service, docs-site/runbooks, docs/superpowers/specs
Predecessor: `2026-11-23-token-jwt-key-persistence-design.md` (PR #7 — persistence/rotation/backup/restore baseline)

## 1. Problem

PR #7 made the JWT signing keys durable (single `keystore.json`, atomic write,
0600) but left the *operational* layer weak:

1. **No audit trail.** No key lifecycle action (create/rotate/retire/delete/
   backup/restore, success or failure) is recorded anywhere. After a key
   compromise there is no evidence of who touched which kid and when.
2. **Backup has no integrity metadata.** A backup is a bare `KeyStoreData`
   dump: no checksum, no backup-format version, no producer/timestamp. A
   truncated or spliced backup that still parses cannot be distinguished from
   the real one.
3. **Restore can destroy live state.** The verified backup overwrites the live
   keystore directly; there is no pre-restore snapshot, no read-back
   verification, no rollback if the post-write state is unusable.
4. **Lost updates on concurrent writes.** `atomic_write` prevents torn files,
   not lost updates. An offline `keytool` (or a second instance) can rewrite
   `keystore.json` between a running service's load and save; the save then
   silently overwrites the external change — potentially deleting a freshly
   rotated key and breaking all new tokens.
5. **Single-instance assumption is implicit.** Nothing documents that scaling
   token-service horizontally is unsupported, nor what would be required.

## 2. Design decisions

### 2.1 Persistent audit ledger (unified event shape, local durable sink)

`keystore-audit.jsonl` lives beside `keystore.json` (0600, one JSON object per
line, `fsync` per record, never rotated by this service). Each line follows the
unified `audit_events` row shape used by audit-service:

```json
{"event_id":"…","event_type":"token.jwt_key.rotated","occurred_at":1764…,
 "trace_id":"…","producer":"token-service",
 "payload":{"actor":"iam-service","action":"rotate","kid":"…",
            "result":"success","related_kid":"…","detail":"…"}}
```

* `actor` — JWT `sub` of the caller (admin API) or `keytool` (CLI).
* `action` — `create|import|rotate|retire|delete|backup|restore|restore_failed|operation_failed`.
* `result` — `success|failed`.
* No field may carry PEM material, key bytes, or credentials. The record type
  has no such fields and tests assert the ledger never contains
  `BEGIN … PRIVATE KEY`.

Why a local ledger and not an outbox → MQ → `audit_events` write:

* the operations that most need auditing (backup / restore) exist **only** in
  the offline `keytool`, which by design has no DB/MQ access;
* keystore state is a file, so a DB-enqueued audit record cannot be in the same
  transaction as the state change ("transactional outbox" does not hold here);
* token-service has no event/outbox infrastructure today; adding outbox +
  publisher + an audit-service `token.#` subscription is a cross-service
  architecture expansion beyond "minimum reasonable change". It is recorded as
  a follow-up decision in the PR report.

Write policy: append is best-effort but never silent — a failed append logs
`ERROR` (kid/action only) and increments
`token_keystore_audit_write_failures_total`.

### 2.2 Single-instance deployment + fail-loud concurrency

Evidence: every compose topology runs exactly one token-service container with
the `token-keys` volume. Therefore: **no distributed lock**. Instead:

* `keystore.json.lock` — `create_new` mutex held across the read-fingerprint →
  rename window. Stale locks older than 10 min are reclaimed; acquisition
  retries ~5 s then fails with a clear "another key operation in progress".
* **Write fingerprint guard** — `KeyStore` records `sha256` of the on-disk file
  at load/first-save; `save()` re-hashes under the lock and refuses to write if
  the disk changed (`KeyStoreError::Stale`). Multi-instance or
  service-vs-keytool races become explicit `409`/CLI errors instead of silent
  key loss.
* Runbook states: horizontal scaling is unsupported; the extension condition is
  moving key state to the DB with row-level locking (or a coordination service).

### 2.3 Backup envelope with integrity + compatibility

```json
{"backup_format_version":1,"created_at":1764…,"producer":"token-service",
 "keystore_sha256":"<hex sha256 of canonical keystore JSON>",
 "keystore":{… KeyStoreData …}}
```

Restore sequence: envelope version check → checksum over the *re-serialized*
`keystore` value → `validate_data` (existing) → pre-restore snapshot of the live
keystore (`keystore.pre-restore.<epoch>.json`, 0600) → locked, fingerprint-
checked atomic save → reload from disk and compare active kid; on read-back
failure restore the snapshot and report `restore_failed`. A failed restore never
leaves the live keystore modified.

Legacy bare-`KeyStoreData` backups are rejected with an explicit message (the
repo is pre-release; no deployed backups exist).

### 2.4 Unchanged

Key status machine (`active`/`retiring`/`retired`), retention window, JWKS
contents, admin route shapes, OpenAPI, deployment manifests, and the
`refresh_runtime_state` panic-on-install-failure behaviour (restart recovers;
documented).

## 3. Error model

New `KeyStoreError` variants: `Stale`, `LockBusy`, `BackupIntegrity`,
`BackupVersion`, `RestoreFailed`. Admin handlers map `Stale`/`LockBusy` to
`409` with an actionable message; CLI exits non-zero with the same message.

## 4. Testing

* audit: records for rotate/retire/delete/backup/restore plus failed variants;
  ledger survives process restart; no PEM material in the file.
* concurrency: two `KeyStore` instances on one path — second save → `Stale`;
  lock contention → `LockBusy` after timeout.
* backup: checksum tamper → `BackupIntegrity`; unknown envelope version →
  `BackupVersion`; corrupted PEM → rejected.
* restore: corrupted backup leaves live keystore byte-identical; success keeps
  old tokens verifiable; pre-restore snapshot written.
* retire/rotate semantics re-asserted (regression guard for PR #7).

## 5. Out of scope

Backup encryption/KEK custody, audit shipping to audit-service, multi-instance
coordination, key material in HSM/KMS, automatic key rotation scheduling.
