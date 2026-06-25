# P2.2 OpenAPI Spec Auto-Dump Design

## Context

P2.1 added `#[utoipa::path]` annotations to every handler in audit/config/file/prue_backend, making OpenAPI a first-class compile-time artifact. The existing `openapi.yaml` files are hand-written and will drift from the utoipa-generated spec.

## Decision

1. **Add a `bin/dump-openapi.rs` binary to each of the 4 services** that calls `ApiDoc::openapi().to_yaml()` and writes the result to `{service}/openapi.yaml`. No server needed — `utoipa::OpenApi` is fully constructible at lib load time.

2. **`scripts/dump-openapi-specs.sh`**: shell script that runs all 4 dump binaries in sequence (avoids port conflicts since each binary links full service code) and writes `openapi.yaml` per service.

3. **CI drift check**: `.gitlab-ci.yml` adds a `spec_generate` job that runs the dump script, then `git diff --exit-code`. If `openapi.yaml` differs from committed version, job fails — forcing the developer to regenerate (which is always correct after a handler change) and commit the new yaml.

## Generated YAML vs. Current Hand-written YAML

The utoipa-generated YAML will differ from the current hand-written spec in some fields:
- No `servers` block (utoipa does not emit it)
- `info.description` comes from `#[openapi(info(...))]` macro arg, not a manual Chinese string
- Tag descriptions may differ
- Schema property ordering may differ

These differences are **expected and correct** — the generated yaml always reflects the actual code.

## Implementation Details

### Per-service binary: `src/bin/dump-openapi.rs`

```rust
use audit_service::openapi::ApiDoc;
use std::fs;

fn main() {
    let yaml = ApiDoc::openapi()
        .to_yaml()
        .expect("failed to serialize OpenAPI to YAML");
    fs::write("openapi.yaml", yaml).expect("failed to write openapi.yaml");
}
```

No new dependencies needed. `utoipa::OpenApi::to_yaml()` is in the standard crate.

### `scripts/dump-openapi-specs.sh`

Runs `cargo run --bin dump-openapi` in each service directory, in sequence.

### CI job: `spec_generate`

```yaml
spec_generate:
  stage: test
  script:
    - chmod +x scripts/dump-openapi-specs.sh
    - scripts/dump-openapi-specs.sh
    - git diff --exit-code -- 'services/*/openapi.yaml'
  artifacts:
    paths:
      - services/*/openapi.yaml
```

Runs after `spec_check` (which ensures `docs/superpowers/specs/` is clean).

## Verification

After implementing, regenerate all 4 yaml files and commit the diffs as the baseline.
