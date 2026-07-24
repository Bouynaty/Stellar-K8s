#!/usr/bin/env bash
# scripts/release-gate.sh
#
# Single-source release validation gate for Stellar-K8s.
# Mirrors every hard gate documented in RELEASE_CHECKLIST.md.
#
# Usage:
#   VERSION=1.2.0 bash scripts/release-gate.sh
#   # or, from a git tag context:
#   bash scripts/release-gate.sh
#
# Exit codes:
#   0  All gates passed — safe to publish
#   1  One or more gates failed — do NOT publish

set -euo pipefail

# ── Colour helpers ────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
RESET='\033[0m'

pass()  { echo -e "  ${GREEN}✓${RESET} $*"; }
fail()  { echo -e "  ${RED}✗${RESET} $*"; FAILURES=$((FAILURES + 1)); }
warn()  { echo -e "  ${YELLOW}⚠${RESET}  $*"; }
section() { echo -e "\n${BOLD}── $* ──${RESET}"; }

FAILURES=0

# ── Resolve version ──────────────────────────────────────────────────────────
if [[ -z "${VERSION:-}" ]]; then
  # Try to derive from the current git tag
  GIT_TAG=$(git describe --exact-match --tags HEAD 2>/dev/null || true)
  if [[ -n "$GIT_TAG" ]]; then
    VERSION="${GIT_TAG#v}"
  else
    echo -e "${RED}ERROR: VERSION is not set and HEAD is not on a git tag.${RESET}"
    echo "  Usage: VERSION=1.2.0 bash scripts/release-gate.sh"
    exit 1
  fi
fi

echo -e "\n${BOLD}Stellar-K8s Release Gate — v${VERSION}${RESET}"
echo "══════════════════════════════════════════════"

# ── Gate 1: Version format ────────────────────────────────────────────────────
section "Gate 1: Version format"
if [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.-]+)?$ ]]; then
  pass "Version '$VERSION' is valid semver"
else
  fail "Version '$VERSION' is NOT valid semver (expected X.Y.Z or X.Y.Z-pre)"
fi

# ── Gate 2: Cargo.toml version matches ───────────────────────────────────────
section "Gate 2: Cargo.toml version"
CARGO_VERSION=$(grep '^version = ' Cargo.toml | head -1 | cut -d'"' -f2)
if [[ "$CARGO_VERSION" == "$VERSION" ]]; then
  pass "Cargo.toml version ($CARGO_VERSION) matches tag"
else
  fail "Cargo.toml version ($CARGO_VERSION) != tag ($VERSION) — update Cargo.toml"
fi

# ── Gate 3: Chart.yaml version matches ───────────────────────────────────────
section "Gate 3: Helm Chart version"
CHART_VERSION=$(grep '^version:' charts/stellar-operator/Chart.yaml | head -1 | awk '{print $2}')
CHART_APP_VERSION=$(grep '^appVersion:' charts/stellar-operator/Chart.yaml | head -1 | awk '{print $2}' | tr -d '"')

if [[ "$CHART_VERSION" == "$VERSION" ]]; then
  pass "Chart.yaml version ($CHART_VERSION) matches tag"
else
  fail "Chart.yaml version ($CHART_VERSION) != tag ($VERSION) — update charts/stellar-operator/Chart.yaml"
fi

if [[ "$CHART_APP_VERSION" == "$VERSION" ]]; then
  pass "Chart.yaml appVersion ($CHART_APP_VERSION) matches tag"
else
  fail "Chart.yaml appVersion ($CHART_APP_VERSION) != tag ($VERSION) — update charts/stellar-operator/Chart.yaml"
fi

# ── Gate 4: CHANGELOG entry exists ───────────────────────────────────────────
section "Gate 4: CHANGELOG entry"
if grep -qE "^## \[?v?${VERSION}\]?" CHANGELOG.md 2>/dev/null; then
  pass "CHANGELOG.md has an entry for v${VERSION}"
else
  fail "CHANGELOG.md is missing an entry for v${VERSION} — add release notes before tagging"
fi

# ── Gate 5: cargo audit ───────────────────────────────────────────────────────
section "Gate 5: cargo audit (security)"
if command -v cargo-audit >/dev/null 2>&1 || cargo install --locked cargo-audit --quiet 2>/dev/null; then
  # Run audit; the .cargo/audit.toml file holds project-level ignores
  if cargo audit --quiet 2>&1 | grep -qE "^error\["; then
    fail "cargo audit found unignored vulnerabilities — resolve or add to .cargo/audit.toml"
  else
    pass "cargo audit passed"
  fi
else
  warn "cargo-audit not available — skipping (install with: cargo install cargo-audit)"
fi

# ── Gate 6: Helm lint ─────────────────────────────────────────────────────────
section "Gate 6: Helm lint"
if command -v helm >/dev/null 2>&1; then
  if helm lint charts/stellar-operator --strict --quiet 2>&1; then
    pass "helm lint --strict passed"
  else
    fail "helm lint --strict failed — fix chart template errors"
  fi
else
  warn "helm not found — skipping (install from https://helm.sh)"
fi

# ── Gate 7: Helm unit tests ───────────────────────────────────────────────────
section "Gate 7: Helm unit tests"
if command -v helm >/dev/null 2>&1 && helm plugin list 2>/dev/null | grep -q unittest; then
  if helm unittest charts/stellar-operator --strict --color 2>&1; then
    pass "helm unittest --strict passed"
  else
    fail "helm unittest failed — fix template test regressions"
  fi
else
  warn "helm-unittest plugin not installed — skipping"
  warn "Install with: helm plugin install https://github.com/helm-unittest/helm-unittest.git"
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "══════════════════════════════════════════════"
if [[ "$FAILURES" -eq 0 ]]; then
  echo -e "${GREEN}${BOLD}✓ All release gates passed — safe to publish v${VERSION}${RESET}"
  echo ""
  echo "  Next steps:"
  echo "    git tag v${VERSION} && git push origin v${VERSION}"
  exit 0
else
  echo -e "${RED}${BOLD}✗ $FAILURES gate(s) FAILED — do NOT publish v${VERSION}${RESET}"
  echo ""
  echo "  Fix all failures above, then re-run:"
  echo "    VERSION=${VERSION} bash scripts/release-gate.sh"
  exit 1
fi
