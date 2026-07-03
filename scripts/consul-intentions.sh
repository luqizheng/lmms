#!/usr/bin/env bash
set -euo pipefail

# Phase 5-A: Consul Connect intention matrix.
# Idempotent: uses `consul intention create -replace` to overwrite.
# Run after `docker-compose up` (Consul must be reachable at localhost:8500).

CONSUL_ADDR="${CONSUL_ADDR:-http://localhost:8500}"
export CONSUL_HTTP_ADDR="$CONSUL_ADDR"

echo "Applying Consul Connect intentions..."

# Allow: api-gateway → all downstream services
consul intention create -replace -allow api-gateway iam-service
consul intention create -replace -allow api-gateway token-service
consul intention create -replace -allow api-gateway notification-service
consul intention create -replace -allow api-gateway audit-service
consul intention create -replace -allow api-gateway config-service
consul intention create -replace -allow api-gateway file-service
consul intention create -replace -allow api-gateway prue_backend

# Allow: token-service → iam-service (fetch_json /internal/users/:id/claims)
consul intention create -replace -allow token-service iam-service

# Allow: notification-service → iam-service (/iam/internal/users/by-{role,dept})
consul intention create -replace -allow notification-service iam-service

# Default deny: everything else
consul intention create -replace -deny '*' '*'

echo "Intentions applied successfully."
consul intention list
