#!/usr/bin/env bash
# scripts/release-gate.sh
#
# Local mirror of .github/workflows/release-gate.yml.
# Only runs gates that are unique to the release gate (CHANGELOG + helm
# unittest). Semver / Cargo.toml / Chart.yaml matching, cargo audit, and
# helm lint are enforced by release.yml and ci.yml — do not re-check here.
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
echo "Unique gates only (changelog + helm unittest)."
echo "semver / Cargo.toml / Chart.yaml / audit / helm lint → release.yml + ci.yml"

# ── Gate 1: CHANGELOG entry exists ───────────────────────────────────────────
section "Gate 1: CHANGELOG entry"
if grep -qE "^## \[?v?${VERSION}\]?" CHANGELOG.md 2>/dev/null; then
  pass "CHANGELOG.md has an entry for v${VERSION}"
else
  fail "CHANGELOG.md is missing an entry for v${VERSION} — add release notes before tagging"
fi

# ── Gate 2: Helm unit tests ───────────────────────────────────────────────────
section "Gate 2: Helm unit tests"
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
  echo -e "${GREEN}${BOLD}✓ Release gates passed — safe to publish v${VERSION}${RESET}"
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
