#!/usr/bin/env bash
set -euo pipefail

echo "========================================"
echo "Running Cache Key Consistency Checks..."
echo "========================================"

ERRORS=0
KEY_REGEX='^(ci|perf|soak|release|chaos|docs|verify|image|dep)-[a-zA-Z0-9_-]+$'
SCOPE_REGEX='^(stellar-k8s-docker|image-build-[a-zA-Z0-9_-]+|perf-docker)$'

# Ensure raw actions/cache is avoided unless specific dimensions are included
while IFS= read -r -d '' file; do
  if grep -q -E 'uses: actions/cache' "$file"; then
    echo "::error file=$file::Raw actions/cache usage detected. Use setup-rust or other wrappers to ensure OS/Arch/Lockfile dimensions are inherently included."
    ERRORS=$((ERRORS + 1))
  fi

  # Validate cache-key / shared-key values (skip GitHub expressions).
  while IFS= read -r line; do
    key=$(echo "$line" | sed -E 's/^[^:]*:[[:space:]]*//; s/["'\'']//g; s/[[:space:]]*$//')
    [[ -z "$key" || "$key" == "default" ]] && continue
    # Allow GitHub Actions expressions (${{ ... }) unchanged.
    [[ "$key" == *'${{'* ]] && continue
    if [[ ! "$key" =~ $KEY_REGEX ]]; then
      echo "::error file=$file::Invalid cache key format: $key. Expected format: <prefix>-<name> where prefix is ci, perf, soak, release, chaos, docs, verify, image, or dep."
      ERRORS=$((ERRORS + 1))
    fi
  done < <(grep -E '^\s*(cache-key|shared-key):' "$file" || true)

  # Check Docker cache scopes
  while IFS= read -r scope; do
    [[ -z "$scope" ]] && continue
    if [[ ! "$scope" =~ $SCOPE_REGEX ]]; then
      echo "::error file=$file::Invalid Docker cache scope: $scope"
      ERRORS=$((ERRORS + 1))
    fi
  done < <(grep -E 'scope=' "$file" | sed -n 's/.*scope=\([^, \"'\'']*\).*/\1/p' || true)
done < <(find .github \( -name "*.yml" -o -name "*.yaml" \) -print0)

if [ "$ERRORS" -gt 0 ]; then
  echo "❌ Found $ERRORS cache key inconsistency issue(s)."
  exit 1
fi

echo "✅ All cache keys follow consistency guidelines."
exit 0
