#!/usr/bin/env bash
set -euo pipefail

echo "========================================"
echo "Running Cache Key Consistency Checks..."
echo "========================================"

ERRORS=0

# Ensure raw actions/cache is avoided unless specific dimensions are included
for file in $(find .github -name "*.yml"); do
  if grep -q -E 'uses: actions/cache' "$file"; then
    echo "::error file=$file::Raw actions/cache usage detected. Use setup-rust or other wrappers to ensure OS/Arch/Lockfile dimensions are inherently included."
    ERRORS=$((ERRORS + 1))
  fi

  # Extract cache-key or shared-key
  keys=$(grep -E 'cache-key:|shared-key:' "$file" | awk -F':' '{print $2}' | tr -d ' "''' || true)
  for key in $keys; do
    # Skip GitHub Actions expression templates (e.g. release-${{ matrix.target }})
    if [[ "$key" == *'${{'* ]]; then
      continue
    fi
    # Valid prefixes
    if [[ ! "$key" =~ ^(ci|perf|soak|release|chaos|docs|verify|image)-[a-zA-Z0-9_-]+$ ]] && [[ "$key" != "default" ]]; then
      echo "::error file=$file::Invalid cache key format: $key. Expected format: <prefix>-<name> where prefix is ci, perf, soak, release, chaos, docs, verify, or image."
      ERRORS=$((ERRORS + 1))
    fi
  done

  # Check Docker cache scopes
  scopes=$(grep -E 'scope=' "$file" | sed -n 's/.*scope=\([^, \"]*\).*/\1/p' | tr -d '\"\''' || true)
  for scope in $scopes; do
    # Remove trailing quotes/whitespace
    scope=$(echo "$scope" | sed 's/["'\'']//g')
    if [[ ! "$scope" =~ ^(stellar-k8s-docker|image-build-[a-zA-Z0-9_-]+|perf-docker)$ ]]; then
      echo "::error file=$file::Invalid Docker cache scope: $scope"
      ERRORS=$((ERRORS + 1))
    fi
  done
done

if [ "$ERRORS" -gt 0 ]; then
  echo "❌ Found $ERRORS cache key inconsistency issue(s)."
  exit 1
fi

echo "✅ All cache keys follow consistency guidelines."
exit 0
