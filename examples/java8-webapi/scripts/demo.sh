#!/usr/bin/env bash
# Live smoke walkthrough for the current LMMS Java SDK.
# Prerequisite: demo-webapp is running and configured with valid gateway credentials.

set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:7000}"

step() { printf "\n==> %s\n" "$1"; }
call() {
  local method="$1"
  local path="$2"
  local body="${3:-}"
  if [[ -n "$body" ]]; then
    curl --fail --silent --show-error -X "$method" "$BASE_URL$path" \
      -H "Content-Type: application/json" -d "$body"
  else
    curl --fail --silent --show-error -X "$method" "$BASE_URL$path"
  fi
  printf "\n"
}

step "IAM user SDK: GET /demo/auth/me"
call GET /demo/auth/me

step "Token user SDK: GET /demo/auth/jwks"
call GET /demo/auth/jwks

step "IAM client SDK: GET /demo/perm/internal (creates a demo role)"
call GET /demo/perm/internal

step "Token user SDK: GET /demo/perm/resource"
call GET /demo/perm/resource

step "Token client SDK: GET /demo/perm/client-jwks"
call GET /demo/perm/client-jwks

step "Notification SDK: GET /demo/notify/unread-count"
call GET /demo/notify/unread-count

printf "\nAll live SDK smoke calls completed successfully.\n"
