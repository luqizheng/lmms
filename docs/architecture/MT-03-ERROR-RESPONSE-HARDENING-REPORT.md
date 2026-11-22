# MT-03 — Error Response Boundary Hardening

**Status:** source fix committed; build/runtime verification pending  
**Branch:** `mt-03-error-response-hardening`

## Finding

`digua_common_error::AppError::into_response` serialized `self.message` for every error kind. The `From<reqwest::Error>` implementation stores `e.to_string()` in that field, and many internal call sites construct `AppError::internal(..., e.to_string())`.

This allowed internal database/transport details to cross the HTTP boundary. Depending on the error, details may include internal hostnames, request URLs/query strings, SQL fragments, or connection metadata.

## Change

- Added a response-safe message policy:
  - `Internal` → `Internal server error`
  - `External` → `Upstream request failed`
  - Client-actionable kinds retain their existing messages.
- Kept detailed diagnostics in the error's Display/source chain for server-side logging.
- Added unit tests for internal/upstream sanitization and preservation of actionable client errors.

## Verification

- [x] Static review of the shared error implementation and HTTP response path.
- [x] Unit test cases added.
- [ ] Cargo build/tests — not run in this environment.
- [ ] HTTP response integration test — pending.
