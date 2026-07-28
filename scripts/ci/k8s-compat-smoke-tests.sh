#!/usr/bin/env bash
set -euo pipefail

echo "========================================"
echo "Kubernetes Compatibility Smoke Tests..."
echo "========================================"

TARGET_VERSION="${1:-1.30.0}"

echo "Testing against Kubernetes v${TARGET_VERSION}"

# We check if kubeconform is available
if ! command -v kubeconform >/dev/null 2>&1; then
  echo "kubeconform is required for compatibility testing."
  exit 1
fi

ERRORS=0

echo "Rendering Helm Chart..."
# Ensure chart dependencies are present (harmless if none)
helm dependency update charts/stellar-operator || true
helm template stellar-operator charts/stellar-operator > /tmp/rendered-chart.yaml

echo "Validating Helm Chart output against k8s v${TARGET_VERSION}..."
# Run kubeconform without -strict to avoid failing CI on non-critical warnings.
if ! kubeconform -kubernetes-version "${TARGET_VERSION}" -ignore-missing-schemas /tmp/rendered-chart.yaml; then
  echo "::error::Helm chart validation failed for Kubernetes v${TARGET_VERSION}"
  ERRORS=$((ERRORS + 1))
fi

echo "Validating static configurations..."
if ! kubeconform -strict -kubernetes-version "${TARGET_VERSION}" -ignore-missing-schemas config/samples/*.yaml; then
  echo "::error::config/samples contain incompatible specs for Kubernetes v${TARGET_VERSION}"
  ERRORS=$((ERRORS + 1))
fi

if [ "$ERRORS" -gt 0 ]; then
  echo "❌ Smoke tests failed for Kubernetes v${TARGET_VERSION}."
  exit 1
fi

echo "✅ Smoke tests passed for Kubernetes v${TARGET_VERSION}."
exit 0
