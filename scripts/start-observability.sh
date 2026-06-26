#!/bin/bash
# Bring up observability stack alongside main infrastructure.
# Requires: docker compose, superpowers network created by main compose.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Ensure superpowers network exists (created by main docker-compose.yml)
if ! docker network ls --format '{{.Name}}' | grep -q '^superpowers$'; then
    echo "ERROR: 'superpowers' network not found."
    echo "  Start main infrastructure first:"
    echo "    docker compose -f infrastructure/docker-compose.yml up -d"
    exit 1
fi

cd "$ROOT_DIR"
docker compose -f infrastructure/docker-compose-observability.yml up -d

echo
echo "=== Observability stack is up ==="
echo "  Prometheus:           http://localhost:9090"
echo "  Grafana:              http://localhost:3001  (admin/admin)"
echo "  Jaeger (traces UI):   http://localhost:16686"
echo "  OTel Collector OTLP:  localhost:4317 (gRPC) / localhost:4318 (HTTP)"
echo
echo "To enable OTel tracing in services, set env var:"
echo "  OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317"
echo
echo "Verify service targets:"
echo "  curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets[].labels.service'"
echo
echo "Stop with:"
echo "  docker compose -f infrastructure/docker-compose-observability.yml down"