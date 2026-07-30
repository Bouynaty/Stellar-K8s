#!/usr/bin/env bash
set -euo pipefail

echo "========================================"
echo "Checking for Stale TODO/FIXME References"
echo "========================================"

# Directories considered critical paths
CRITICAL_PATHS=(
  ".github/"
  "scripts/"
  "charts/"
  "src/"
)

# Paths that document the TODO policy itself (or generate issues) and would
# otherwise self-match on the words TODO/FIXME.
EXCLUDE_REGEX='(^scripts/ci/check-stale-todos\.sh$|^scripts/archive/)'

ERRORS=0

echo "Scanning critical paths: ${CRITICAL_PATHS[*]}"

for dir in "${CRITICAL_PATHS[@]}"; do
  if [ ! -d "$dir" ]; then
    continue
  fi

  while IFS= read -r match; do
    if [ -z "$match" ]; then continue; fi
    # match format: file:line:content
    file=$(echo "$match" | cut -d':' -f1)
    line=$(echo "$match" | cut -d':' -f2)
    content=$(echo "$match" | cut -d':' -f3-)

    if echo "$file" | grep -E -q "$EXCLUDE_REGEX"; then
      continue
    fi

    # Valid scopes: TODO(#123), TODO(@user), TODO(exempt: reason)
    if ! echo "$content" | grep -E -q '\b(TODO|FIXME)\((#[0-9]+|@[a-zA-Z0-9_-]+|exempt:[^)]+)\)'; then
      echo "::error file=$file,line=$line::Stale or improperly formatted TODO/FIXME found. Use TODO(#[issue]), TODO(@[username]), or TODO(exempt: [reason])."
      echo "  Line: $content"
      ERRORS=$((ERRORS + 1))
    fi

  done < <(grep -rnE '\b(TODO|FIXME)\b' "$dir" || true)

done

if [ "$ERRORS" -gt 0 ]; then
  echo "Found $ERRORS stale TODO/FIXME references."
  exit 1
fi

echo "All TODO/FIXME references in critical paths are properly documented."
exit 0
