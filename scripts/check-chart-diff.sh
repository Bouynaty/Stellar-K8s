#!/usr/bin/env bash
# check-chart-diff.sh - Detect unintended Helm chart template drift
#
# This script ensures that Helm chart rendering is deterministic by:
# 1. Rendering the chart with default values
# 2. Comparing against a stored baseline (if available)
# 3. Flagging any unexpected changes
#
# Related: #1145 - Implement chart render diff checks to catch unintended template drift

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CHART_DIR="$PROJECT_ROOT/charts/stellar-operator"
BASELINE_FILE="$PROJECT_ROOT/.cache/chart-render-baseline.yaml"
TEMP_DIR=$(mktemp -d)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

ERRORS=0
WARNINGS=0

cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

echo "→ Checking Helm chart render consistency..."
echo ""

# Check if chart directory exists
if [[ ! -d "$CHART_DIR" ]]; then
    echo "ERROR: Chart directory not found: $CHART_DIR"
    exit 1
fi

# Check if helm is installed
if ! command -v helm >/dev/null 2>&1; then
    echo "ERROR: helm is not installed"
    exit 1
fi

# Create temp output file
RENDERED="$TEMP_DIR/rendered.yaml"

echo "→ Rendering Helm chart with default values..."
if ! helm template stellar-operator "$CHART_DIR" > "$RENDERED" 2>&1; then
    echo -e "  ${RED}✗${NC} Failed to render chart"
    cat "$RENDERED"
    exit 1
fi
echo -e "  ${GREEN}✓${NC} Chart rendered successfully"

# Count resources
MANIFEST_COUNT=$(grep -c "^---" "$RENDERED" || echo "0")
echo "  Resources rendered: $MANIFEST_COUNT"
echo ""

# Check if baseline exists
if [[ -f "$BASELINE_FILE" ]]; then
    echo "→ Comparing against stored baseline..."
    
    # Use diff to compare
    if diff -u "$BASELINE_FILE" "$RENDERED" > "$TEMP_DIR/diff.txt" 2>&1; then
        echo -e "  ${GREEN}✓${NC} Chart render matches baseline (no drift)"
    else
        echo -e "  ${RED}✗${NC} Chart render differs from baseline!"
        echo ""
        echo "  Diff summary:"
        DIFF_LINES=$(wc -l < "$TEMP_DIR/diff.txt")
        echo "    Lines changed: $DIFF_LINES"
        echo ""
        echo "  Showing first 50 lines of diff:"
        head -50 "$TEMP_DIR/diff.txt" | sed 's/^/    /'
        echo ""
        
        ((WARNINGS++))
        
        # Check if this is a PR (git available)
        if git rev-parse --git-dir > /dev/null 2>&1; then
            echo -e "  ${YELLOW}⚠${NC}  Baseline mismatch detected"
            echo "     If this change is intentional, update the baseline:"
            echo "     cp $RENDERED $BASELINE_FILE"
        fi
    fi
else
    echo "→ No baseline found, creating one..."
    mkdir -p "$(dirname "$BASELINE_FILE")"
    cp "$RENDERED" "$BASELINE_FILE"
    echo -e "  ${GREEN}✓${NC} Created baseline at $BASELINE_FILE"
    echo -e "  ${YELLOW}ℹ${NC}  Future runs will compare against this baseline"
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Chart Render Diff Check Summary:"
echo -e "  Errors:   ${RED}$ERRORS${NC}"
echo -e "  Warnings: ${YELLOW}$WARNINGS${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [[ $ERRORS -gt 0 ]]; then
    echo ""
    echo "❌ Chart render diff check FAILED"
    exit 1
elif [[ $WARNINGS -gt 0 ]]; then
    echo ""
    echo "⚠️  Chart render diff check detected drift"
    echo "   Review the diff above and update baseline if intentional"
    exit 0
else
    echo ""
    echo "✅ Chart render is consistent"
    exit 0
fi

#!/usr/bin/env bash
# check-chart-diff.sh - Detect unintended Helm chart template drift
#
# This script ensures that Helm chart rendering is deterministic by:
# 1. Rendering the chart with default values
# 2. Comparing against a stored baseline (if available)
# 3. Flagging any unexpected changes
#
# Related: #1145 - Implement chart render diff checks to catch unintended template drift

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CHART_DIR="$PROJECT_ROOT/charts/stellar-operator"
BASELINE_FILE="$PROJECT_ROOT/.cache/chart-render-baseline.yaml"
TEMP_DIR=$(mktemp -d)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

ERRORS=0
WARNINGS=0

cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

echo "→ Checking Helm chart render consistency..."
echo ""

# Check if chart directory exists
if [[ ! -d "$CHART_DIR" ]]; then
    echo "ERROR: Chart directory not found: $CHART_DIR"
    exit 1
fi

# Check if helm is installed
if ! command -v helm >/dev/null 2>&1; then
    echo "ERROR: helm is not installed"
    exit 1
fi

# Create temp output file
RENDERED="$TEMP_DIR/rendered.yaml"

echo "→ Rendering Helm chart with default values..."
if ! helm template stellar-operator "$CHART_DIR" > "$RENDERED" 2>&1; then
    echo -e "  ${RED}✗${NC} Failed to render chart"
    cat "$RENDERED"
    exit 1
fi
echo -e "  ${GREEN}✓${NC} Chart rendered successfully"

# Count resources
MANIFEST_COUNT=$(grep -c "^---" "$RENDERED" || echo "0")
echo "  Resources rendered: $MANIFEST_COUNT"
echo ""

# Check if baseline exists
if [[ -f "$BASELINE_FILE" ]]; then
    echo "→ Comparing against stored baseline..."
    
    # Use diff to compare
    if diff -u "$BASELINE_FILE" "$RENDERED" > "$TEMP_DIR/diff.txt" 2>&1; then
        echo -e "  ${GREEN}✓${NC} Chart render matches baseline (no drift)"
    else
        echo -e "  ${RED}✗${NC} Chart render differs from baseline!"
        echo ""
        echo "  Diff summary:"
        DIFF_LINES=$(wc -l < "$TEMP_DIR/diff.txt")
        echo "    Lines changed: $DIFF_LINES"
        echo ""
        echo "  Showing first 50 lines of diff:"
        head -50 "$TEMP_DIR/diff.txt" | sed 's/^/    /'
        echo ""
        
        WARNINGS=$((WARNINGS + 1))
        
        # Check if this is a PR (git available)
        if git rev-parse --git-dir > /dev/null 2>&1; then
            echo -e "  ${YELLOW}⚠${NC}  Baseline mismatch detected"
            echo "     If this change is intentional, update the baseline:"
            echo "     cp $RENDERED $BASELINE_FILE"
        fi
    fi
else
    echo "→ No baseline found, creating one..."
    mkdir -p "$(dirname "$BASELINE_FILE")"
    cp "$RENDERED" "$BASELINE_FILE"
    echo -e "  ${GREEN}✓${NC} Created baseline at $BASELINE_FILE"
    echo -e "  ${YELLOW}ℹ${NC}  Future runs will compare against this baseline"
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Chart Render Diff Check Summary:"
echo -e "  Errors:   ${RED}$ERRORS${NC}"
echo -e "  Warnings: ${YELLOW}$WARNINGS${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [[ $ERRORS -gt 0 ]]; then
    echo ""
    echo "❌ Chart render diff check FAILED"
    exit 1
elif [[ $WARNINGS -gt 0 ]]; then
    echo ""
    echo "⚠️  Chart render diff check detected drift"
    echo "   Review the diff above and update baseline if intentional"
    exit 0
else
    echo ""
    echo "✅ Chart render is consistent"
    exit 0
fi
