#!/usr/bin/env bash
# scripts/preflight.sh — Build preflight check for required binaries and minimum versions.
#
# Verifies that every tool required to build, test, and operate Stellar-K8s is
# installed at or above the minimum supported version. Exits 0 only when all
# required checks pass. Optional tools are reported but do not cause failure.
#
# Usage:
#   bash scripts/preflight.sh            # normal check (exits 1 on any required failure)
#   bash scripts/preflight.sh --strict   # also fail on optional-tool warnings
#   bash scripts/preflight.sh --ci       # machine-readable output (plain text, no colours)
#   bash scripts/preflight.sh --labels   # also check required GitHub repo labels
#   REPO=OtowoOrg/Stellar-K8s bash scripts/preflight.sh --labels
#
# Environment variables:
#   PREFLIGHT_SKIP   Comma-separated list of tool names to skip (e.g. PREFLIGHT_SKIP=k6,helm)
#
# Tool version requirements are pinned here and must be kept in sync with:
#   - scripts/setup-mac.sh (pinned install versions)
#   - .github/actions/setup-rust/action.yml (toolchain)
#   - .github/workflows/ci.yml (version pins)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib/errors.sh
source "${SCRIPT_DIR}/lib/errors.sh"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# ── CLI flag parsing ─────────────────────────────────────────────────────────

STRICT=false
CI_MODE=false
CHECK_LABELS=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --strict)  STRICT=true ;;
    --ci)      CI_MODE=true ;;
    --labels)  CHECK_LABELS=true ;;
    -h|--help)
      echo "Usage: $0 [--strict] [--ci] [--labels]"
      echo "  --strict  Fail on optional-tool warnings too"
      echo "  --ci      Plain output (no colour codes)"
      echo "  --labels  Also verify required GitHub repo labels exist"
      exit 0
      ;;
    *) echo "Unknown flag: $1" >&2; exit 1 ;;
  esac
  shift
done

# ── Colour helpers ───────────────────────────────────────────────────────────

if [[ "${CI_MODE}" == "true" ]] || [[ ! -t 1 ]]; then
  RED="" GREEN="" YELLOW="" CYAN="" BOLD="" RESET=""
else
  RED='\033[0;31m' GREEN='\033[0;32m' YELLOW='\033[1;33m'
  CYAN='\033[0;36m' BOLD='\033[1m' RESET='\033[0m'
fi

pass()  { echo -e "  ${GREEN}✓${RESET} $*"; }
fail()  { echo -e "  ${RED}✗${RESET} $*"; }
warn()  { echo -e "  ${YELLOW}⚠${RESET} $*"; }
info()  { echo -e "  ${CYAN}→${RESET} $*"; }
header(){ echo -e "\n${BOLD}$*${RESET}"; }

# ── Skip list ────────────────────────────────────────────────────────────────

IFS=',' read -ra _SKIP_LIST <<< "${PREFLIGHT_SKIP:-}"
should_skip() {
  local name="$1"
  for s in "${_SKIP_LIST[@]:-}"; do
    [[ "${s}" == "${name}" ]] && return 0
  done
  return 1
}

# ── Version comparison ───────────────────────────────────────────────────────

# version_ge <actual> <minimum>
# Returns 0 (true) if actual >= minimum using numeric segment comparison.
version_ge() {
  local actual="$1" minimum="$2"
  # Strip leading 'v', 'go', etc.
  actual="${actual#[vVgG]}"
  minimum="${minimum#[vVgG]}"

  IFS='.' read -ra A <<< "$actual"
  IFS='.' read -ra M <<< "$minimum"

  local i
  for (( i=0; i<${#M[@]}; i++ )); do
    local a="${A[$i]:-0}" m="${M[$i]:-0}"
    # Strip non-numeric suffix (e.g. "1rc2" → "1")
    a="${a%%[^0-9]*}" m="${m%%[^0-9]*}"
    a="${a:-0}"       m="${m:-0}"
    if   (( 10#$a > 10#$m )); then return 0
    elif (( 10#$a < 10#$m )); then return 1
    fi
  done
  return 0  # equal
}

# ── Check runner ─────────────────────────────────────────────────────────────

REQUIRED_PASS=0
REQUIRED_FAIL=0
OPTIONAL_WARN=0

# check_tool <name> <version_flag> <min_version> <required|optional> <extract_regex> [install_hint]
check_tool() {
  local name="$1"
  local version_flag="$2"
  local min_version="$3"
  local required="$4"
  local extract_regex="$5"
  local install_hint="${6:-}"

  if should_skip "${name}"; then
    info "${name}: skipped (PREFLIGHT_SKIP)"
    return 0
  fi

  if ! command -v "${name}" >/dev/null 2>&1; then
    if [[ "${required}" == "required" ]]; then
      fail "${name}: NOT FOUND (required)"
      [[ -n "${install_hint}" ]] && echo "       Hint: ${install_hint}"
      REQUIRED_FAIL=$(( REQUIRED_FAIL + 1 ))
    else
      warn "${name}: not found (optional)"
      [[ -n "${install_hint}" ]] && echo "       Hint: ${install_hint}"
      OPTIONAL_WARN=$(( OPTIONAL_WARN + 1 ))
    fi
    return 0
  fi

  local raw_version
  raw_version=$("${name}" ${version_flag} 2>&1 | head -1) || true
  local actual_version
  actual_version=$(echo "${raw_version}" | sed -E "${extract_regex}" 2>/dev/null || true)

  if [[ -z "${actual_version}" ]]; then
    warn "${name}: could not parse version from: ${raw_version}"
    return 0
  fi

  if version_ge "${actual_version}" "${min_version}"; then
    pass "${name} ${actual_version} (≥ ${min_version})"
    REQUIRED_PASS=$(( REQUIRED_PASS + 1 ))
  else
    local msg="${name} ${actual_version} is below minimum ${min_version}"
    if [[ "${required}" == "required" ]]; then
      fail "${msg} (required)"
      [[ -n "${install_hint}" ]] && echo "       Hint: ${install_hint}"
      REQUIRED_FAIL=$(( REQUIRED_FAIL + 1 ))
    else
      warn "${msg} (optional)"
      OPTIONAL_WARN=$(( OPTIONAL_WARN + 1 ))
    fi
  fi
}

# ── Checks ───────────────────────────────────────────────────────────────────

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Stellar-K8s build preflight check"
echo "  repo: ${REPO_ROOT}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

header "Required tools"

check_tool "cargo" "--version" "1.88.0" "required" \
  's/.*cargo ([0-9]+\.[0-9]+\.[0-9]+).*/\1/' \
  "Install Rust via rustup: https://rustup.rs/"

check_tool "rustc" "--version" "1.88.0" "required" \
  's/rustc ([0-9]+\.[0-9]+\.[0-9]+).*/\1/' \
  "Install Rust via rustup: https://rustup.rs/"

check_tool "git" "--version" "2.30.0" "required" \
  's/git version ([0-9]+\.[0-9]+\.[0-9]+).*/\1/' \
  "Install git: https://git-scm.com/"

check_tool "docker" "--version" "20.10.0" "required" \
  's/.*Docker version ([0-9]+\.[0-9]+\.[0-9]+).*/\1/' \
  "Install Docker Engine: https://docs.docker.com/engine/install/"

check_tool "kubectl" "version --client --short" "1.28.0" "required" \
  's/.*: v([0-9]+\.[0-9]+\.[0-9]+).*/\1/' \
  "Install kubectl: https://kubernetes.io/docs/tasks/tools/"

check_tool "helm" "version --short" "3.14.0" "required" \
  's/v([0-9]+\.[0-9]+\.[0-9]+).*/\1/' \
  "Install Helm 3: https://helm.sh/docs/intro/install/"

check_tool "kind" "--version" "0.20.0" "required" \
  's/kind v([0-9]+\.[0-9]+\.[0-9]+).*/\1/' \
  "Install kind: https://kind.sigs.k8s.io/docs/user/quick-start/"

header "Optional tools"

check_tool "shellcheck" "--version" "0.9.0" "optional" \
  's/.*version: ([0-9]+\.[0-9]+\.[0-9]+).*/\1/' \
  "Install shellcheck: https://github.com/koalaman/shellcheck#installing"

check_tool "python3" "--version" "3.10.0" "optional" \
  's/Python ([0-9]+\.[0-9]+\.[0-9]+).*/\1/' \
  "Install Python 3: https://www.python.org/downloads/"

check_tool "gh" "--version" "2.30.0" "optional" \
  's/gh version ([0-9]+\.[0-9]+\.[0-9]+).*/\1/' \
  "Install GitHub CLI: https://cli.github.com/"

check_tool "k6" "version" "0.50.0" "optional" \
  's/.*([0-9]+\.[0-9]+\.[0-9]+).*/\1/' \
  "Install k6: https://k6.io/docs/getting-started/installation/"

check_tool "pre-commit" "--version" "3.0.0" "optional" \
  's/pre-commit ([0-9]+\.[0-9]+\.[0-9]+).*/\1/' \
  "Install: pip install pre-commit"

# ── Rust toolchain components ─────────────────────────────────────────────────

header "Rust toolchain components"

sk8s_step "rustfmt" "Checking rustfmt"
if command -v rustfmt >/dev/null 2>&1; then
  pass "rustfmt $(rustfmt --version 2>&1 | head -1)"
else
  fail "rustfmt not found — run: rustup component add rustfmt"
  REQUIRED_FAIL=$(( REQUIRED_FAIL + 1 ))
fi

sk8s_step "clippy" "Checking cargo-clippy"
if cargo clippy --version >/dev/null 2>&1; then
  pass "clippy $(cargo clippy --version 2>&1 | head -1)"
else
  fail "clippy not found — run: rustup component add clippy"
  REQUIRED_FAIL=$(( REQUIRED_FAIL + 1 ))
fi

# ── GitHub label check (--labels flag) ───────────────────────────────────────

if [[ "${CHECK_LABELS}" == "true" ]]; then
  header "GitHub repo labels"

  REQUIRED_LABELS=("ci" "security" "stellar-wave" "maintenance" "hygiene")

  # Detect repo from REPO env or git remote
  LABEL_REPO="${REPO:-}"
  if [[ -z "${LABEL_REPO}" ]]; then
    LABEL_REPO=$(git remote get-url origin 2>/dev/null \
      | sed -E 's|.*github\.com[:/]||; s|\.git$||') || true
  fi

  if [[ -z "${LABEL_REPO}" ]]; then
    warn "REPO not set and could not detect from git remote — skipping label check"
  elif ! command -v gh >/dev/null 2>&1; then
    warn "'gh' CLI not found — skipping label check. Install: https://cli.github.com/"
  elif ! gh auth status >/dev/null 2>&1; then
    warn "Not authenticated with gh CLI — run 'gh auth login' to enable label checks"
  else
    echo "  Checking labels in ${LABEL_REPO}"
    existing=$(gh label list --repo "${LABEL_REPO}" --json name --limit 200 \
      | python3 -c "import sys,json; print('\n'.join(l['name'] for l in json.load(sys.stdin)))" \
      2>/dev/null || true)

    for label in "${REQUIRED_LABELS[@]}"; do
      if echo "${existing}" | grep -qx "${label}"; then
        pass "label '${label}' exists"
      else
        warn "label '${label}' missing"
        OPTIONAL_WARN=$(( OPTIONAL_WARN + 1 ))
      fi
    done
  fi
fi

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [[ ${REQUIRED_FAIL} -gt 0 ]]; then
  echo -e "  ${RED}✗ Preflight FAILED${RESET}: ${REQUIRED_FAIL} required tool(s) missing or outdated."
  echo "  Fix the issues above before running 'make dev-setup' or 'make build'."
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  exit 1
fi

if [[ ${OPTIONAL_WARN} -gt 0 ]]; then
  if [[ "${STRICT}" == "true" ]]; then
    echo -e "  ${RED}✗ Preflight FAILED (strict)${RESET}: ${OPTIONAL_WARN} optional tool(s) missing or outdated."
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    exit 1
  else
    echo -e "  ${YELLOW}⚠ Preflight PASSED with warnings${RESET}: ${OPTIONAL_WARN} optional tool(s) not available."
    echo "  Some developer features may be unavailable. Run with --strict to fail on warnings."
  fi
else
  echo -e "  ${GREEN}✓ All preflight checks passed${RESET} (${REQUIRED_PASS} tools verified)."
fi

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
