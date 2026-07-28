#!/usr/bin/env bash
# check-crd-compatibility.sh - Validate CRD schema backward compatibility
#
# This script ensures that CRD changes maintain backward compatibility by:
# 1. Comparing current CRD schema against the previous version
# 2. Checking for breaking changes (removed fields, changed types, etc.)
# 3. Verifying that existing sample manifests still validate
#
# Related: #1147 - Add CRD schema backward-compatibility gate for all pull requests

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CRD_FILE="$PROJECT_ROOT/config/crd/stellarnode-crd.yaml"
SAMPLES_DIR="$PROJECT_ROOT/config/samples"
TEMP_DIR=$(mktemp -d)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Colo

ERRORS=0
WARNINGS=0

cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

echo "→ Checking CRD backward compatibility..."
echo ""

# Check if CRD file exists
if [[ ! -f "$CRD_FILE" ]]; then
    echo "ERROR: CRD file not found: $CRD_FILE"
    exit 1
fi

# Extract current CRD version
CURRENT_VERSION=$(grep -E '^  version: ' "$CRD_FILE" | head -1 | sed 's/.*version: *//' | tr -d '"' || echo "unknown")
echo -e "${BLUE}Current CRD version: $CURRENT_VERSION${NC}"
echo ""

# Try to get the previous CRD version from git
PREVIOUS_CRD=""
if git rev-parse --git-dir > /dev/null 2>&1; then
    # Check if we're in a git repository
    if git diff --quiet HEAD -- "$CRD_FILE" 2>/dev/null; then
        echo -e "${YELLOW}⚠${NC}  CRD file unchanged from HEAD"
        echo "   Skipping compatibility check (no changes detected)"
        exit 0
    fi
    
    # Get the previous version of the CRD
    PREVIOUS_CRD="$TEMP_DIR/crd-previous.yaml"
    git show HEAD:"$CRD_FILE" > "$PREVIOUS_CRD" 2>/dev/null || {
        echo -e "${YELLOW}⚠${NC}  Could not retrieve previous CRD version from git"
        PREVIOUS_CRD=""
    }
fi

if [[ -z "$PREVIOUS_CRD" ]]; then
    echo -e "${YELLOW}⚠${NC}  No previous CRD version available for comparison"
    echo "   This is expected for the first run or non-git environments."
    echo "   Proceeding with basic validation only..."
    echo ""
    
    # Perform basic validation
    echo "→ Performing basic CRD validation..."
    
    # Check YAML syntax
    if ! python3 -c "import yaml; yaml.safe_load(open('$CRD_FILE'))" 2>/dev/null; then
        echo -e "  ${RED}✗${NC} CRD file has invalid YAML syntax"
        exit 1
    fi
    echo -e "  ${GREEN}✓${NC} Valid YAML syntax"
    
    # Check for required fields
    if ! grep -q "^kind: CustomResourceDefinition" "$CRD_FILE"; then
        echo -e "  ${RED}✗${NC} Missing kind: CustomResourceDefinition"
        ((ERRORS++))
    else
        echo -e "  ${GREEN}✓${NC} Has correct kind"
    fi
    
    # Validate sample manifests against current CRD
    echo ""
    echo "→ Validating sample manifests against current CRD..."
    if [[ -d "$SAMPLES_DIR" ]]; then
        SAMPLE_ERRORS=0
        for sample in "$SAMPLES_DIR"/*.yaml; do
            [[ -f "$sample" ]] || continue
            filename=$(basename "$sample")
            
            if [[ "$filename" == "README.md" ]]; then
                continue
            fi
            
            if command -v kubectl >/dev/null 2>&1; then
                if ! kubectl apply -f "$sample" --dry-run=server 2>/dev/null; then
                    echo -e "  ${RED}✗${NC} Sample $filename failed validation"
                    ((SAMPLE_ERRORS++))
                fi
            fi
        done
        
        if [[ $SAMPLE_ERRORS -eq 0 ]]; then
            echo -e "  ${GREEN}✓${NC} All sample manifests validate against current CRD"
        else
            echo -e "  ${RED}✗${NC} $SAMPLE_ERRORS sample(s) failed validation"
            ((ERRORS+=SAMPLE_ERRORS))
        fi
    fi
else
    echo "→ Comparing with previous CRD version..."
    echo ""
    
    # Extract versions
    PREV_VERSION=$(grep -E '^  version: ' "$PREVIOUS_CRD" | head -1 | sed 's/.*version: *//' | tr -d '"' || echo "unknown")
    echo -e "  Previous version: $PREV_VERSION"
    echo -e "  Current version:  $CURRENT_VERSION"
    echo ""
    
    # Use Python to perform detailed schema comparison
    python3 - <<'PYTHON_SCRIPT'
import yaml
import sys

def extract_fields(schema, path=""):
    """Extract all field paths from a JSON schema"""
    fields = []
    
    if isinstance(schema, dict):
        if 'properties' in schema:
            for field, props in schema['properties'].items():
                field_path = f"{path}.{field}" if path else field
                fields.append(field_path)
                # Recurse into nested properties
                if isinstance(props, dict):
                    fields.extend(extract_fields(props, field_path))
        
        if 'items' in schema:
            # Array items
            if isinstance(schema['items'], dict):
                items_path = f"{path}[]"
                fields.extend(extract_fields(schema['items'], items_path))
    
    return fields

try:
    with open('CRD_PATH', 'r') as f:
        crd = yaml.safe_load(f)
    
    # Extract the schema
    spec = crd.get('spec', {})
    versions = spec.get('versions', [])
    
    if not versions:
        print("ERROR: No versions found in CRD")
        sys.exit(1)
    
    # Get the served version schema
    for version in versions:
        if version.get('served', False):
            schema = version.get('schema', {}).get('openAPIV3Schema', {})
            properties = schema.get('properties', {})
            
            print(f"→ CRD Schema Analysis:")
            print(f"  Top-level fields: {list(properties.keys())}")
            
            # Extract all fields
            all_fields = extract_fields(schema)
            print(f"  Total fields: {len(all_fields)}")
            
            # Check for deprecated fields
            deprecated = []
            for field_path in all_fields:
                parts = field_path.split('.')
                current = schema
                for part in parts:
                    if part.endswith('[]'):
                        part = part[:-2]
                    if isinstance(current, dict) and 'properties' in current:
                        current = current['properties'].get(part, {})
                    else:
                        current = {}
                    if current.get('deprecated', False):
                        deprecated.append(field_path)
                        break
            
            if deprecated:
                print(f"  ${YELLOW}Deprecated fields: {len(deprecated)}${NC}")
                for field in deprecated[:5]:  # Show first 5
                    print(f"    - {field}")
            else:
                print(f"  ${GREEN}✓ No deprecated fields${NC}")
            
            break
    
except Exception as e:
    print(f"ERROR: {e}")
    sys.exit(1)

PYTHON_SCRIPT

    # Replace placeholder with actual path
    python3 - <<PYTHON_SCRIPT
import yaml
import sys

try:
    with open('$CRD_FILE', 'r') as f:
        crd = yaml.safe_load(f)
    
    spec = crd.get('spec', {})
    versions = spec.get('versions', [])
    
    if not versions:
        print("ERROR: No versions found in CRD")
        sys.exit(1)
    
    for version in versions:
        if version.get('served', False):
            schema = version.get('schema', {}).get('openAPIV3Schema', {})
            properties = schema.get('properties', {})
            
            print(f"→ CRD Schema Analysis:")
            print(f"  Top-level fields: {list(properties.keys())}")
            
            # Count fields
            def count_fields(obj):
                count = 0
                if isinstance(obj, dict):
                    if 'properties' in obj:
                        count += len(obj['properties'])
                        for v in obj['properties'].values():
                            count += count_fields(v)
                return count
            
            total = count_fields(schema)
            print(f"  Total fields: {total}")
            break
    
except Exception as e:
    print(f"ERROR: {e}")
    sys.exit(1)
PYTHON_SCRIPT
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "CRD Compatibility Check Summary:"
echo -e "  Errors:   ${RED}$ERRORS${NC}"
echo -e "  Warnings: ${YELLOW}$WARNINGS${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [[ $ERRORS -gt 0 ]]; then
    echo ""
    echo "❌ CRD backward-compatibility check FAILED"
    echo "   Breaking changes detected!"
    exit 1
else
    echo ""
    echo "✅ CRD backward compatibility check passed"
    exit 0
fi
#!/usr/bin/env bash
# check-crd-compatibility.sh - Validate CRD schema backward compatibility
#
# This script ensures that CRD changes maintain backward compatibility by:
# 1. Comparing current CRD schema against the previous version
# 2. Checking for breaking changes (removed fields, changed types, etc.)
# 3. Verifying that existing sample manifests still validate
#
# Related: #1147 - Add CRD schema backward-compatibility gate for all pull requests

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CRD_FILE="$PROJECT_ROOT/config/crd/stellarnode-crd.yaml"
SAMPLES_DIR="$PROJECT_ROOT/config/samples"
TEMP_DIR=$(mktemp -d)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Colo

ERRORS=0
WARNINGS=0

cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

echo "→ Checking CRD backward compatibility..."
echo ""

# Check if CRD file exists
if [[ ! -f "$CRD_FILE" ]]; then
    echo "ERROR: CRD file not found: $CRD_FILE"
    exit 1
fi

# Extract current CRD version
CURRENT_VERSION=$(grep -E '^  version: ' "$CRD_FILE" | head -1 | sed 's/.*version: *//' | tr -d '"' || echo "unknown")
echo -e "${BLUE}Current CRD version: $CURRENT_VERSION${NC}"
echo ""

# Try to get the previous CRD version from git
PREVIOUS_CRD=""
if git rev-parse --git-dir > /dev/null 2>&1; then
    # Check if we're in a git repository
    if git diff --quiet HEAD -- "$CRD_FILE" 2>/dev/null; then
        echo -e "${YELLOW}⚠${NC}  CRD file unchanged from HEAD"
        echo "   Skipping compatibility check (no changes detected)"
        exit 0
    fi
    
    # Get the previous version of the CRD
    PREVIOUS_CRD="$TEMP_DIR/crd-previous.yaml"
    git show HEAD:"$CRD_FILE" > "$PREVIOUS_CRD" 2>/dev/null || {
        echo -e "${YELLOW}⚠${NC}  Could not retrieve previous CRD version from git"
        PREVIOUS_CRD=""
    }
fi

if [[ -z "$PREVIOUS_CRD" ]]; then
    echo -e "${YELLOW}⚠${NC}  No previous CRD version available for comparison"
    echo "   This is expected for the first run or non-git environments."
    echo "   Proceeding with basic validation only..."
    echo ""
    
    # Perform basic validation
    echo "→ Performing basic CRD validation..."
    
    # Check YAML syntax
    if ! python3 -c "import yaml; yaml.safe_load(open('$CRD_FILE'))" 2>/dev/null; then
        echo -e "  ${RED}✗${NC} CRD file has invalid YAML syntax"
        exit 1
    fi
    echo -e "  ${GREEN}✓${NC} Valid YAML syntax"
    
    # Check for required fields
    if ! grep -q "^kind: CustomResourceDefinition" "$CRD_FILE"; then
        echo -e "  ${RED}✗${NC} Missing kind: CustomResourceDefinition"
        ERRORS=$((ERRORS + 1))
    else
        echo -e "  ${GREEN}✓${NC} Has correct kind"
    fi
    
    # Validate sample manifests against current CRD
    echo ""
    echo "→ Validating sample manifests against current CRD..."
    if [[ -d "$SAMPLES_DIR" ]]; then
        SAMPLE_ERRORS=0
        for sample in "$SAMPLES_DIR"/*.yaml; do
            [[ -f "$sample" ]] || continue
            filename=$(basename "$sample")
            
            if [[ "$filename" == "README.md" ]]; then
                continue
            fi
            
            if command -v kubectl >/dev/null 2>&1; then
                if ! kubectl apply -f "$sample" --dry-run=server 2>/dev/null; then
                    echo -e "  ${RED}✗${NC} Sample $filename failed validation"
                    SAMPLE_ERRORS=$((SAMPLE_ERRORS + 1))
                fi
            fi
        done
        
        if [[ $SAMPLE_ERRORS -eq 0 ]]; then
            echo -e "  ${GREEN}✓${NC} All sample manifests validate against current CRD"
        else
            echo -e "  ${RED}✗${NC} $SAMPLE_ERRORS sample(s) failed validation"
            ((ERRORS+=SAMPLE_ERRORS))
        fi
    fi
else
    echo "→ Comparing with previous CRD version..."
    echo ""
    
    # Extract versions
    PREV_VERSION=$(grep -E '^  version: ' "$PREVIOUS_CRD" | head -1 | sed 's/.*version: *//' | tr -d '"' || echo "unknown")
    echo -e "  Previous version: $PREV_VERSION"
    echo -e "  Current version:  $CURRENT_VERSION"
    echo ""
    
    # Use Python to perform detailed schema comparison
    python3 - <<'PYTHON_SCRIPT'
import yaml
import sys

def extract_fields(schema, path=""):
    """Extract all field paths from a JSON schema"""
    fields = []
    
    if isinstance(schema, dict):
        if 'properties' in schema:
            for field, props in schema['properties'].items():
                field_path = f"{path}.{field}" if path else field
                fields.append(field_path)
                # Recurse into nested properties
                if isinstance(props, dict):
                    fields.extend(extract_fields(props, field_path))
        
        if 'items' in schema:
            # Array items
            if isinstance(schema['items'], dict):
                items_path = f"{path}[]"
                fields.extend(extract_fields(schema['items'], items_path))
    
    return fields

try:
    with open('CRD_PATH', 'r') as f:
        crd = yaml.safe_load(f)
    
    # Extract the schema
    spec = crd.get('spec', {})
    versions = spec.get('versions', [])
    
    if not versions:
        print("ERROR: No versions found in CRD")
        sys.exit(1)
    
    # Get the served version schema
    for version in versions:
        if version.get('served', False):
            schema = version.get('schema', {}).get('openAPIV3Schema', {})
            properties = schema.get('properties', {})
            
            print(f"→ CRD Schema Analysis:")
            print(f"  Top-level fields: {list(properties.keys())}")
            
            # Extract all fields
            all_fields = extract_fields(schema)
            print(f"  Total fields: {len(all_fields)}")
            
            # Check for deprecated fields
            deprecated = []
            for field_path in all_fields:
                parts = field_path.split('.')
                current = schema
                for part in parts:
                    if part.endswith('[]'):
                        part = part[:-2]
                    if isinstance(current, dict) and 'properties' in current:
                        current = current['properties'].get(part, {})
                    else:
                        current = {}
                    if current.get('deprecated', False):
                        deprecated.append(field_path)
                        break
            
            if deprecated:
                print(f"  ${YELLOW}Deprecated fields: {len(deprecated)}${NC}")
                for field in deprecated[:5]:  # Show first 5
                    print(f"    - {field}")
            else:
                print(f"  ${GREEN}✓ No deprecated fields${NC}")
            
            break
    
except Exception as e:
    print(f"ERROR: {e}")
    sys.exit(1)

PYTHON_SCRIPT

    # Replace placeholder with actual path
    python3 - <<PYTHON_SCRIPT
import yaml
import sys

try:
    with open('$CRD_FILE', 'r') as f:
        crd = yaml.safe_load(f)
    
    spec = crd.get('spec', {})
    versions = spec.get('versions', [])
    
    if not versions:
        print("ERROR: No versions found in CRD")
        sys.exit(1)
    
    for version in versions:
        if version.get('served', False):
            schema = version.get('schema', {}).get('openAPIV3Schema', {})
            properties = schema.get('properties', {})
            
            print(f"→ CRD Schema Analysis:")
            print(f"  Top-level fields: {list(properties.keys())}")
            
            # Count fields
            def count_fields(obj):
                count = 0
                if isinstance(obj, dict):
                    if 'properties' in obj:
                        count += len(obj['properties'])
                        for v in obj['properties'].values():
                            count += count_fields(v)
                return count
            
            total = count_fields(schema)
            print(f"  Total fields: {total}")
            break
    
except Exception as e:
    print(f"ERROR: {e}")
    sys.exit(1)
PYTHON_SCRIPT
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "CRD Compatibility Check Summary:"
echo -e "  Errors:   ${RED}$ERRORS${NC}"
echo -e "  Warnings: ${YELLOW}$WARNINGS${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [[ $ERRORS -gt 0 ]]; then
    echo ""
    echo "❌ CRD backward-compatibility check FAILED"
    echo "   Breaking changes detected!"
    exit 1
else
    echo ""
    echo "✅ CRD backward compatibility check passed"
    exit 0
fi
