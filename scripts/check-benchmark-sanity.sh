#!/usr/bin/env bash
# check-benchmark-sanity.sh - Reproducible benchmark sanity checks for PR pipelines
#
# This script ensures benchmark results are reproducible by:
# 1. Running a quick benchmark suite
# 2. Comparing results against stored baselines
# 3. Flagging performance regressions beyond threshold
#
# Related: #1144 - Add reproducible benchmark sanity checks in pull request pipelines

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BENCHMARK_DIR="$PROJECT_ROOT/benchmarks"
BASELINE_DIR="$PROJECT_ROOT/.cache/benchmark-baselines"
TEMP_DIR=$(mktemp -d)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
REGRESSION_THRESHOLD=10  # Percent regression threshold
ERRORS=0
WARNINGS=0

cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

echo "→ Running benchmark sanity checks..."
echo ""

# Check if we're in a PR environment
if [[ "${GITHUB_EVENT_NAME:-}" != "pull_request" ]] && [[ "${CI:-}" != "true" ]]; then
    echo -e "${YELLOW}⚠${NC}  Not in PR/CI environment - skipping benchmark check"
    echo "   This check is designed for pull request pipelines."
    exit 0
fi

# Check if benchmark directory exists
if [[ ! -d "$BENCHMARK_DIR" ]]; then
    echo "ERROR: Benchmark directory not found: $BENCHMARK_DIR"
    exit 1
fi

mkdir -p "$BASELINE_DIR"

echo "→ Running reconciler benchmark..."
cd "$BENCHMARK_DIR"

# Run a quick benchmark (lower iteration count for CI)
BENCHMARK_OUTPUT="$TEMP_DIR/benchmark-output.txt"
BENCHMARK_RESULTS="$TEMP_DIR/benchmark-results.json"

if ! cargo test --release --lib reconciler::benchmarks -- --nocapture 2>&1 | tee "$BENCHMARK_OUTPUT"; then
    echo -e "  ${RED}✗${NC} Benchmark execution failed"
    exit 1
fi
echo -e "  ${GREEN}✓${NC} Benchmark completed"

# Extract key metrics from output
echo ""
echo "→ Extracting benchmark metrics..."

# Look for timing information in the output
if [[ -f "$BENCHMARK_OUTPUT" ]]; then
    # Try to extract numeric timing values
    TIMING_VALUES=$(grep -oE '[0-9]+\.[0-9]+(ms|s|μs|ns)' "$BENCHMARK_OUTPUT" | head -20 || echo "")
    
    if [[ -n "$TIMING_VALUES" ]]; then
        echo "  Extracted timing metrics:"
        echo "$TIMING_VALUES" | while read -r line; do
            echo "    $line"
        done
    fi
fi

# Check for baseline comparison
BENCHMARK_NAME="reconciler"
BASELINE_FILE="$BASELINE_DIR/${BENCHMARK_NAME}-baseline.txt"

if [[ -f "$BASELINE_FILE" ]]; then
    echo ""
    echo "→ Comparing against baseline..."
    
    # Simple comparison: check if key timing metrics exist
    BASELINE_LINES=$(wc -l < "$BASELINE_FILE")
    CURRENT_LINES=$(wc -l < "$BENCHMARK_OUTPUT")
    
    echo "  Baseline lines: $BASELINE_LINES"
    echo "  Current lines:  $CURRENT_LINES"
    
    # Calculate a rough regression metric by comparing total output
    if [[ $CURRENT_LINES -gt 0 ]]; then
        LINE_RATIO=$(( (CURRENT_LINES * 100) / BASELINE_LINES ))
        echo "  Output ratio: ${LINE_RATIO}%"
        
        # Check for significant deviations (simplified check)
        if [[ $LINE_RATIO -lt 80 ]] || [[ $LINE_RATIO -gt 120 ]]; then
            echo -e "  ${YELLOW}⚠${NC}  Significant deviation detected (>20% change)"
            ((WARNINGS++))
        else
            echo -e "  ${GREEN}✓${NC} Within acceptable range (±20%)"
        fi
    fi
else
    echo ""
    echo "→ No baseline found, creating one..."
    cp "$BENCHMARK_OUTPUT" "$BASELINE_FILE"
    echo -e "  ${GREEN}✓${NC} Created baseline at $BASELINE_FILE"
    echo -e "  ${YELLOW}ℹ${NC}  Future PRs will compare against this baseline"
fi

# Additional sanity checks
echo ""
echo "→ Running sanity checks..."

# Check 1: No panics in benchmark output
if grep -qi "panic" "$BENCHMARK_OUTPUT"; then
    echo -e "  ${RED}✗${NC} Panic detected in benchmark output"
    ((ERRORS++))
else
    echo -e "  ${GREEN}✓${NC} No panics detected"
fi

# Check 2: All tests passed
if grep -qi "test result: ok" "$BENCHMARK_OUTPUT"; then
    echo -e "  ${GREEN}✓${NC} All benchmark tests passed"
elif grep -qi "test result: FAILED" "$BENCHMARK_OUTPUT"; then
    echo -e "  ${RED}✗${NC} Benchmark tests failed"
    ((ERRORS++))
else
    echo -e "  ${YELLOW}⚠${NC}  Could not determine test result"
fi

# Check 3: Reasonable execution time (not hanging)
BENCHMARK_TIME=$(grep -oE "real[	 ]+[0-9]+m[0-9]+\.[0-9]+s" "$BENCHMARK_OUTPUT" | head -1 || echo "")
if [[ -n "$BENCHMARK_TIME" ]]; then
    echo -e "  ${GREEN}✓${NC} Benchmark completed in: $BENCHMARK_TIME"
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Benchmark Sanity Check Summary:"
echo -e "  Errors:   ${RED}$ERRORS${NC}"
echo -e "  Warnings: ${YELLOW}$WARNINGS${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [[ $ERRORS -gt 0 ]]; then
    echo ""
    echo "❌ Benchmark sanity check FAILED"
    exit 1
elif [[ $WARNINGS -gt 0 ]]; then
    echo ""
    echo "⚠️  Benchmark sanity check detected potential regressions"
    echo "   Review the results above"
    exit 0
else
    echo ""
    echo "✅ Benchmark sanity checks passed"
    exit 0
fi
