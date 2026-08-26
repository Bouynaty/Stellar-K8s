#!/usr/bin/env bash
# Automated database migration tests (issue #1317).
#
# Provisions nothing itself — CI (or the developer) must provide DATABASE_URL
# pointing at an isolated Postgres instance. Safe test credentials only.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "ERROR: DATABASE_URL is required."
  echo "Example: postgres://stellar:stellar_test@127.0.0.1:5432/stellar_migration_test"
  exit 1
fi

echo "→ Loading bundled SQL migrations from db/migrations"
mapfile -t UP_FILES < <(ls -1 db/migrations/*.up.sql)
if [[ ${#UP_FILES[@]} -lt 1 ]]; then
  echo "ERROR: no up migrations found"
  exit 1
fi

echo "→ Running Rust migration harness (fresh + existing-data + rollback)"
export DATABASE_URL
export STELLAR_MIGRATION_TEST=1
cargo test --test db_migration_harness --features rest-api,metrics,admission-webhook,k8s-v1-30 -- --nocapture

echo "✓ Database migration tests passed"
