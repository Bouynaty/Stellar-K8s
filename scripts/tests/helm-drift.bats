#!/usr/bin/env bats
# scripts/tests/helm-drift.bats — Tests for scripts/check-helm-drift.sh (#1045)
#
# Run:  bats scripts/tests/helm-drift.bats
# Requires: bats-core, helm, python3

setup() {
  REPO_ROOT="$(cd "${BATS_TEST_DIRNAME}/../.." && pwd)"
  DRIFT="${REPO_ROOT}/scripts/check-helm-drift.sh"
  GOLDEN_DIR="${REPO_ROOT}/charts/stellar-operator/rendered"
  TEMPLATE_DIR="${REPO_ROOT}/charts/stellar-operator/templates"
  export REPO_ROOT DRIFT GOLDEN_DIR TEMPLATE_DIR
}

_require_helm() {
  if ! command -v helm >/dev/null 2>&1; then
    skip "helm is not installed"
  fi
}

# Restore a template that a test mutated in place.
_restore() {
  if [ -n "${MUTATED:-}" ] && [ -f "${BATS_TEST_TMPDIR}/backup" ]; then
    cp "${BATS_TEST_TMPDIR}/backup" "${MUTATED}"
  fi
}

teardown() {
  _restore
}

# ---------------------------------------------------------------------------
# Invocation
# ---------------------------------------------------------------------------

@test "--help prints usage without running helm" {
  run bash "${DRIFT}" --help
  [ "$status" -eq 0 ]
  [[ "$output" == *"drift detection"* ]]
}

@test "--list prints every configured profile" {
  run bash "${DRIFT}" --list
  [ "$status" -eq 0 ]
  [[ "$output" == *"default"* ]]
  [[ "$output" == *"ha"* ]]
  [[ "$output" == *"production"* ]]
  [[ "$output" == *"development"* ]]
  [[ "$output" == *"dr-cross-region"* ]]
}

@test "an unknown argument exits 2" {
  run bash "${DRIFT}" --nonsense
  [ "$status" -eq 2 ]
}

# ---------------------------------------------------------------------------
# Golden files
# ---------------------------------------------------------------------------

@test "every configured profile has a committed golden file" {
  run bash "${DRIFT}" --list
  [ "$status" -eq 0 ]
  while IFS= read -r profile; do
    [ -s "${GOLDEN_DIR}/${profile}.yaml" ] || {
      echo "missing golden for profile: ${profile}"
      return 1
    }
  done <<< "$output"
}

@test "goldens are normalised (documents separated and key-sorted)" {
  grep -q '^---$' "${GOLDEN_DIR}/default.yaml"
  # sort-manifests.py sorts mapping keys, so apiVersion precedes kind.
  run grep -n -m1 -E '^(apiVersion|kind):' "${GOLDEN_DIR}/default.yaml"
  [[ "$output" == *"apiVersion:"* ]]
}

# ---------------------------------------------------------------------------
# Drift detection
# ---------------------------------------------------------------------------

@test "a clean tree reports no drift" {
  _require_helm
  run bash "${DRIFT}"
  [ "$status" -eq 0 ]
  [[ "$output" == *"match the committed goldens"* ]]
}

@test "a template change is reported as drift and exits 1" {
  _require_helm
  MUTATED="${TEMPLATE_DIR}/service.yaml"
  export MUTATED
  cp "${MUTATED}" "${BATS_TEST_TMPDIR}/backup"

  # Add a label that must show up in the rendered Service.
  printf '\n# drift probe\n' >> "${MUTATED}"
  sed -i 's/^  type: .*/  type: NodePort/' "${MUTATED}"

  run bash "${DRIFT}" --profile default
  [ "$status" -eq 1 ]
  [[ "$output" == *"drifted from the golden file"* ]]
  [[ "$output" == *"make helm-drift-update"* ]]
}

@test "a render failure is reported rather than silently passing" {
  _require_helm
  MUTATED="${TEMPLATE_DIR}/service.yaml"
  export MUTATED
  cp "${MUTATED}" "${BATS_TEST_TMPDIR}/backup"

  printf '\n{{ .Values.thisKeyDoesNotExist.andNorDoesThis }}\n' >> "${MUTATED}"

  run bash "${DRIFT}" --profile default
  [ "$status" -eq 1 ]
  [[ "$output" == *"render failed"* ]] || [[ "$output" == *"drifted"* ]]
}

@test "--profile restricts the run to a single profile" {
  _require_helm
  run bash "${DRIFT}" --profile development
  [ "$status" -eq 0 ]
  [[ "$output" == *"development"* ]]
  [[ "$output" != *"production:"* ]]
}

# ---------------------------------------------------------------------------
# Regression guards for the bugs this gate was added to catch
# ---------------------------------------------------------------------------

@test "the production values profile renders (regression: crossRegion nil pointer)" {
  _require_helm
  run helm template stellar-operator "${REPO_ROOT}/charts/stellar-operator" \
    -f "${REPO_ROOT}/charts/stellar-operator/examples/values-production.yaml"
  [ "$status" -eq 0 ]
}

@test "enabling DR alone does not break rendering" {
  _require_helm
  run helm template stellar-operator "${REPO_ROOT}/charts/stellar-operator" \
    --set featureFlags.enableDr=true
  [ "$status" -eq 0 ]
}

@test "the cross-region bridge renders when DR and crossRegion are both on" {
  _require_helm
  run helm template stellar-operator "${REPO_ROOT}/charts/stellar-operator" \
    --set featureFlags.enableDr=true \
    --set crossRegion.enabled=true \
    --set crossRegion.peerClusters[0].clusterId=us-west-2 \
    --set crossRegion.peerClusters[0].enabled=true \
    --set crossRegion.peerClusters[0].endpoint=api.us-west-2.example.com
  [ "$status" -eq 0 ]
  [[ "$output" == *"stellar-bridge-us-west-2"* ]]
  [[ "$output" == *"ExternalName"* ]]
}
