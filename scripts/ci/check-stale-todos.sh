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

ERRORS=0

echo "Scanning critical paths: ${CRITICAL_PATHS[*]}"

# Find all files in critical paths, ignoring binary and known large directories if any
# We use grep with line numbers and file names
# The regex matches TODO or FIXME without a valid scope.
# Valid scopes: TODO(issue-number), TODO(@username), TODO(exempt: reason)
# Invalid: TODO:, TODO, FIXME

for dir in "${CRITICAL_PATHS[@]}"; do
  if [ ! -d "$dir" ]; then
    continue
  fi

  # grep -rn 'TODO\|FIXME' "$dir" 
  # We look for lines with TODO or FIXME, then use a regex in awk/sed or bash to validate them.
  
  while IFS= read -r match; do
    if [ -z "$match" ]; then continue; fi
    # match format: file:line:content
    file=$(echo "$match" | cut -d':' -f1)
    line=$(echo "$match" | cut -d':' -f2)
    content=$(echo "$match" | cut -d':' -f3-)

    # If the content matches TODO( or FIXME( with valid inner text, it's fine.
    # Otherwise, it's stale.
    # Valid pattern example: TODO([#0-9]+|@[a-zA-Z0-9_-]+|exempt:.*)
    
    # We strip out valid ones and see if TODO/FIXME still exists in an invalid form
    # We can just extract all TODO/FIXMEs from the line and check each.
    
    # Use grep -q to check if the line contains a valid marker. If it does, we assume it's valid?
    # Better: check if it contains ANY invalid markers.
    invalid_found=false
    
    # We check if it matches the valid formats.
    # To be strictly safe and portable, we can just use python or perl.
    # Or simply: if it doesn't match the valid pattern, it's invalid.
    if ! echo "$content" | grep -E -q '\b(TODO|FIXME)\((#[0-9]+|@[a-zA-Z0-9_-]+|exempt:[^)]+)\)'; then
      invalid_found=true
    fi
    
    if [ "$invalid_found" = true ]; then
      echo "::error file=$file,line=$line::Stale or improperly formatted TODO/FIXME found. Use TODO(#[issue]), TODO(@[username]), or TODO(exempt: [reason])."
      echo "  Line: $content"
      ERRORS=$((ERRORS + 1))
    fi

  done < <(grep -rnE '\b(TODO|FIXME)\b' "$dir" \
             --exclude='check-stale-todos.sh' \
             --exclude-dir='.git' \
             || true)

done

if [ "$ERRORS" -gt 0 ]; then
  echo "❌ Found $ERRORS stale TODO/FIXME references."
  exit 1
fi

echo "✅ All TODO/FIXME references in critical paths are properly documented."
exit 0
