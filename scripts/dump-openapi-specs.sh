#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

dump_service() {
    local svc="$1"
    local dir="$ROOT_DIR/services/$svc"
    echo "=== Dumping OpenAPI for $svc ==="
    (cd "$dir" && cargo run --bin dump-openapi 2>&1)
    echo "--- $svc/openapi.yaml written ---"
}

dump_service audit-service
dump_service config-service
dump_service file-service
dump_service prue_backend

echo "=== All OpenAPI specs dumped ==="
