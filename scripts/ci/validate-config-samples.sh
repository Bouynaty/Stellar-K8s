#!/usr/bin/env bash
set -euo pipefail

echo "========================================"
echo "Validating Configuration Samples..."
echo "========================================"

# Requires: kubeconform (or similar) installed in CI

if ! command -v kubeconform >/dev/null 2>&1; then
  echo "kubeconform is not installed. Skipping schema validation."
  # In CI, we will ensure kubeconform is available.
  if [ "${CI:-}" == "true" ]; then
    exit 1
  fi
  exit 0
fi

ERRORS=0

# Validate examples/ and config/samples/
for dir in examples config/samples; do
  if [ -d "$dir" ]; then
    echo "Validating YAML files in $dir..."
    # We ignore CRDs since kubeconform needs custom schemas for them.
    # We pass -ignore-missing-schemas to not fail on unknown CRs like StellarNode,
    # unless we explicitly provide the CRD schema.
    find "$dir" -name "*.yaml" -type f | while read -r file; do
      if ! kubeconform -strict -ignore-missing-schemas "$file"; then
        echo "::error file=$file::Schema validation failed for $file"
        ERRORS=$((ERRORS + 1))
      fi
    done
  fi
done

if [ "$ERRORS" -gt 0 ]; then
  echo "❌ Found $ERRORS schema validation issue(s)."
  exit 1
fi

echo "✅ All configuration samples are valid."
exit 0
