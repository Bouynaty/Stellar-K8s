#!/usr/bin/env bash
# scripts/cleanup.sh — Single supported repository cleanup tool.
#
# Replaces the removed one-off helpers:
#   - scripts/cleanup_root.sh
#   - scripts/organize_scripts.sh
#   - scripts/archive/* batch / hygiene scripts
#
# Usage:
#   ./scripts/cleanup.sh              # apply cleanup
#   ./scripts/cleanup.sh --dry-run    # report only
#   make cleanup
#   make cleanup DRY_RUN=1
#
# Exit 0 on success. Never deletes tracked source files for scratch cleanup.
# Exits non-zero if obsolete archive/cleanup helpers have been reintroduced.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

# shellcheck source=scripts/lib/errors.sh
source "${SCRIPT_DIR}/lib/errors.sh"

DRY_RUN=0
REMOVED=0
SKIPPED=0
OBSOLETE_FOUND=0

usage() {
  cat <<'EOF'
Usage: cleanup.sh [OPTIONS]

Single supported repository cleanup tool for Stellar-K8s.

Removes common local scratch artifacts from the repository root and verifies
that obsolete archive-script paths are not reintroduced.

Options:
  --dry-run    Report actions without deleting anything
  -h, --help   Show this help

Makefile:
  make cleanup
  make cleanup DRY_RUN=1
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1; shift ;;
    -h | --help) usage; exit 0 ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

# Root-level scratch files historically cleaned by cleanup_root.sh.
ROOT_SCRATCH=(
  build_errors.txt
  cargo_check.log
  check.log
  gh_log.txt
  log.txt
  rendered-output.yaml
)

# Obsolete paths that must not return (archive / one-off cleanup helpers).
OBSOLETE_PATHS=(
  scripts/archive
  scripts/cleanup_root.sh
  scripts/organize_scripts.sh
  scripts/lib/batch.sh
)

remove_path() {
  local path="$1"
  if [[ ! -e "${path}" ]]; then
    SKIPPED=$((SKIPPED + 1))
    return 0
  fi
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "  [dry-run] would remove: ${path}"
  else
    rm -f "${path}"
    echo "  removed: ${path}"
  fi
  REMOVED=$((REMOVED + 1))
}

sk8s_step "scratch cleanup" "Removing root-level scratch artifacts"
for f in "${ROOT_SCRATCH[@]}"; do
  remove_path "${f}"
done

sk8s_step "obsolete paths" "Ensuring removed archive/cleanup helpers stay gone"
for p in "${OBSOLETE_PATHS[@]}"; do
  if [[ -e "${p}" ]]; then
    sk8s_error "obsolete path still present: ${p}"
    echo "  Hint: delete it; use scripts/cleanup.sh as the only cleanup entrypoint." >&2
    OBSOLETE_FOUND=1
  else
    echo "  ok: absent ${p}"
  fi
done

sk8s_step "summary" "Cleanup complete"
if [[ "${DRY_RUN}" -eq 1 ]]; then
  echo "  mode: dry-run"
fi
echo "  scratch removals: ${REMOVED}"
echo "  already clean: ${SKIPPED}"
echo ""
echo "Supported cleanup entrypoint: scripts/cleanup.sh (make cleanup)"
echo "Repository health gate remains: make health / scripts/repo-health.sh"

if [[ "${OBSOLETE_FOUND}" -ne 0 ]]; then
  sk8s_fail "Obsolete archive/cleanup paths are present" \
    "Remove them and keep scripts/cleanup.sh as the single cleanup tool."
fi
