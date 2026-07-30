#!/usr/bin/env bash
# check-stale-samples.sh - Detect and optionally fix stale sample manifests
#
# This script validates that sample manifests in config/samples/ are:
# 1. Syntactically valid YAML
# 2. Conform to the current CRD schema
# 3. Not missing required fields
#
# Related: #1146 - Create automated stale sample manifest detector and fixer

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SAMPLES_DIR="$PROJECT_ROOT/config/samples"
CRD_FILE="$PROJECT_ROOT/config/crd/stellarnode-crd.yaml"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

ERRORS=0
WARNINGS=0

echo "→ Checking sample manifests in $SAMPLES_DIR..."
echo ""

# Check if samples directory exists
if [[ ! -d "$SAMPLES_DIR" ]]; then
    echo "ERROR: Samples directory not found: $SAMPLES_DIR"
    exit 1
fi

# Check if CRD file exists
if [[ ! -f "$CRD_FILE" ]]; then
    echo "ERROR: CRD file not found: $CRD_FILE"
    exit 1
fi

# Validate each sample file
for sample in "$SAMPLES_DIR"/*.yaml; do
    [[ -f "$sample" ]] || continue
    
    filename=$(basename "$sample")
    echo "Checking: $filename"
    
    # Skip README
    if [[ "$filename" == "README.md" ]]; then
        continue
    fi
    
    # Check 1: Valid YAML syntax
    if ! python3 -c "import yaml; yaml.safe_load(open('$sample'))" 2>/dev/null; then
        echo -e "  ${RED}✗${NC} Invalid YAML syntax"
        ((ERRORS++))
        continue
    fi
    echo -e "  ${GREEN}✓${NC} Valid YAML syntax"
    
    # Check 2: Has apiVersion and kind
    if ! grep -q "^apiVersion:" "$sample"; then
        echo -e "  ${RED}✗${NC} Missing apiVersion field"
        ((ERRORS++))
    else
        echo -e "  ${GREEN}✓${NC} Has apiVersion"
    fi
    
    if ! grep -q "^kind:" "$sample"; then
        echo -e "  ${RED}✗${NC} Missing kind field"
        ((ERRORS++))
    else
        echo -e "  ${GREEN}✓${NC} Has kind"
    fi
    
    # Check 3: Has metadata.name
    if ! grep -q "^  name:" "$sample"; then
        echo -e "  ${YELLOW}⚠${NC}  Missing metadata.name"
        ((WARNINGS++))
    else
        echo -e "  ${GREEN}✓${NC} Has metadata.name"
    fi
    
    # Check 4: Has metadata.namespace
    if ! grep -q "^  namespace:" "$sample"; then
        echo -e "  ${YELLOW}⚠${NC}  Missing metadata.namespace"
        ((WARNINGS++))
    else
        echo -e "  ${GREEN}✓${NC} Has metadata.namespace"
    fi
    
    # Check 5: Validate against CRD schema if kubectl is available
    if command -v kubectl >/dev/null 2>&1; then
        # Dry-run validation against the CRD
        if kubectl apply -f "$sample" --dry-run=server 2>/dev/null; then
            echo -e "  ${GREEN}✓${NC} Passes CRD validation (dry-run)"
        else
            echo -e "  ${RED}✗${NC} Failed CRD validation (dry-run)"
            ((ERRORS++))
        fi
    else
        echo -e "  ${YELLOW}⚠${NC}  kubectl not available - skipping CRD validation"
    fi
    
    echo ""
done

# Summary
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Sample Manifest Check Summary:"
echo -e "  Errors:   ${RED}$ERRORS${NC}"
echo -e "  Warnings: ${YELLOW}$WARNINGS${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [[ $ERRORS -gt 0 ]]; then
    echo ""
    echo "❌ Sample manifest check FAILED"
    echo "   Fix the errors above or run: make regenerate-samples"
    exit 1
elif [[ $WARNINGS -gt 0 ]]; then
    echo ""
    echo "⚠️  Sample manifest check passed with warnings"
    exit 0
else
    echo ""
    echo "✅ All sample manifests are valid and up-to-date"
    exit 0
fi
#!/usr/bin/env bash
# check-stale-samples.sh - Detect and optionally fix stale sample manifests
#
# This script validates that sample manifests in config/samples/ are:
# 1. Syntactically valid YAML
# 2. Conform to the current CRD schema
# 3. Not missing required fields
#
# Related: #1146 - Create automated stale sample manifest detector and fixer

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SAMPLES_DIR="$PROJECT_ROOT/config/samples"
CRD_FILE="$PROJECT_ROOT/config/crd/stellarnode-crd.yaml"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

ERRORS=0
WARNINGS=0

echo "→ Checking sample manifests in $SAMPLES_DIR..."
echo ""

# Check if samples directory exists
if [[ ! -d "$SAMPLES_DIR" ]]; then
    echo "ERROR: Samples directory not found: $SAMPLES_DIR"
    exit 1
fi

# Check if CRD file exists
if [[ ! -f "$CRD_FILE" ]]; then
    echo "ERROR: CRD file not found: $CRD_FILE"
    exit 1
fi

# Validate each sample file
for sample in "$SAMPLES_DIR"/*.yaml; do
    [[ -f "$sample" ]] || continue
    
    filename=$(basename "$sample")
    echo "Checking: $filename"
    
    # Skip README
    if [[ "$filename" == "README.md" ]]; then
        continue
    fi
    
    # Check 1: Valid YAML syntax (multi-document samples supported)
    if ! python3 -c "import yaml; list(yaml.safe_load_all(open('$sample')))" 2>/dev/null; then
        echo -e "  ${RED}✗${NC} Invalid YAML syntax"
        ERRORS=$((ERRORS + 1))
        continue
    fi
    echo -e "  ${GREEN}✓${NC} Valid YAML syntax"
    
    # Check 2: Has apiVersion and kind
    if ! grep -q "^apiVersion:" "$sample"; then
        echo -e "  ${RED}✗${NC} Missing apiVersion field"
        ERRORS=$((ERRORS + 1))
    else
        echo -e "  ${GREEN}✓${NC} Has apiVersion"
    fi
    
    if ! grep -q "^kind:" "$sample"; then
        echo -e "  ${RED}✗${NC} Missing kind field"
        ERRORS=$((ERRORS + 1))
    else
        echo -e "  ${GREEN}✓${NC} Has kind"
    fi
    
    # Check 3: Has metadata.name
    if ! grep -q "^  name:" "$sample"; then
        echo -e "  ${YELLOW}⚠${NC}  Missing metadata.name"
        WARNINGS=$((WARNINGS + 1))
    else
        echo -e "  ${GREEN}✓${NC} Has metadata.name"
    fi
    
    # Check 4: Has metadata.namespace
    if ! grep -q "^  namespace:" "$sample"; then
        echo -e "  ${YELLOW}⚠${NC}  Missing metadata.namespace"
        WARNINGS=$((WARNINGS + 1))
    else
        echo -e "  ${GREEN}✓${NC} Has metadata.namespace"
    fi
    
    # Check 5: Validate against a live cluster only (server dry-run needs API server)
    if command -v kubectl >/dev/null 2>&1 && kubectl cluster-info >/dev/null 2>&1; then
        if kubectl apply -f "$sample" --dry-run=server >/dev/null 2>&1; then
            echo -e "  ${GREEN}✓${NC} Passes CRD validation (dry-run)"
        else
            echo -e "  ${RED}✗${NC} Failed CRD validation (dry-run)"
            ERRORS=$((ERRORS + 1))
        fi
    else
        echo -e "  ${YELLOW}⚠${NC}  kubectl cluster not available - skipping CRD validation"
    fi
    
    echo ""
done

# Summary
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Sample Manifest Check Summary:"
echo -e "  Errors:   ${RED}$ERRORS${NC}"
echo -e "  Warnings: ${YELLOW}$WARNINGS${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [[ $ERRORS -gt 0 ]]; then
    echo ""
    echo "❌ Sample manifest check FAILED"
    echo "   Fix the errors above or run: make regenerate-samples"
    exit 1
elif [[ $WARNINGS -gt 0 ]]; then
    echo ""
    echo "⚠️  Sample manifest check passed with warnings"
    exit 0
else
    echo ""
    echo "✅ All sample manifests are valid and up-to-date"
    exit 0
fi
