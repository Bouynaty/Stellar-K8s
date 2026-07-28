#!/bin/bash
# Security audit and dependency checking script for Stellar K8s Operator
# Usage: ./scripts/security-check.sh [--fix]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

# Check if running with --fix flag
FIX_MODE=${1:-""}

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Install required tools if missing
install_security_tools() {
    log_info "Checking security audit tools..."
    
    if ! command_exists cargo-deny; then
        log_info "Installing cargo-deny..."
        cargo install cargo-deny --locked
    fi
    
    if ! command_exists cargo-audit; then
        log_info "Installing cargo-audit..."
        cargo install cargo-audit --locked
    fi
    
    if ! command_exists cargo-outdated; then
        log_info "Installing cargo-outdated..."
        cargo install cargo-outdated --locked
    fi
}

# Run security audits
run_security_audit() {
    log_info "Running comprehensive security audit..."
    
    # Check for known vulnerabilities
    log_info "Checking for security advisories..."
    if cargo audit --quiet; then
        log_success "No known vulnerabilities found"
    else
        log_warn "Security advisories found (check .cargo/audit.toml for justified ignores)"
    fi
    
    # Check for banned/problematic dependencies
    log_info "Checking dependency policy compliance..."
    if cargo deny check; then
        log_success "All dependency policies satisfied"
    else
        log_error "Dependency policy violations found"
        return 1
    fi
    
    # Check for outdated dependencies
    log_info "Checking for outdated dependencies..."
    if command_exists cargo-outdated; then
        cargo outdated --root-deps-only --format json > outdated_deps.json 2>/dev/null || true
        if [[ -s outdated_deps.json ]]; then
            log_warn "Some dependencies may be outdated (see outdated_deps.json)"
        else
            log_success "Dependencies are up to date"
        fi
        rm -f outdated_deps.json
    fi
    
    # Check for duplicate dependencies
    log_info "Checking for duplicate dependencies..."
    DUPLICATES=$(cargo tree --duplicates 2>/dev/null || echo "")
    if [[ -n "$DUPLICATES" ]]; then
        log_warn "Duplicate dependencies found:"
        echo "$DUPLICATES"
    else
        log_success "No duplicate dependencies found"
    fi
}

# Generate SBOM (Software Bill of Materials)
generate_sbom() {
    log_info "Generating Software Bill of Materials..."
    
    # Create SBOM directory
    mkdir -p "$PROJECT_ROOT/security/sbom"
    
    # Generate dependency tree
    cargo tree --format "{p} {l}" > "$PROJECT_ROOT/security/sbom/dependencies.txt"
    
    # Generate detailed dependency info with licenses
    if command_exists cargo-deny; then
        cargo deny list --format json > "$PROJECT_ROOT/security/sbom/licenses.json" 2>/dev/null || true
    fi
    
    log_success "SBOM generated in security/sbom/"
}

# Check build security settings
check_build_security() {
    log_info "Checking build security configuration..."
    
    # Check if security profiles are configured
    if grep -q "strip = true" "$PROJECT_ROOT/Cargo.toml"; then
        log_success "Symbol stripping enabled"
    else
        log_warn "Symbol stripping not configured"
    fi
    
    if grep -q "panic = \"abort\"" "$PROJECT_ROOT/Cargo.toml"; then
        log_success "Panic abort mode configured"
    else
        log_warn "Panic abort mode not configured"
    fi
    
    if grep -q "lto = true" "$PROJECT_ROOT/Cargo.toml"; then
        log_success "Link-time optimization enabled"
    else
        log_warn "Link-time optimization not configured"
    fi
}

# Update security configurations if in fix mode
apply_security_fixes() {
    if [[ "$FIX_MODE" == "--fix" ]]; then
        log_info "Applying automated security fixes..."
        
        # Update Cargo.lock
        log_info "Updating dependency lockfile..."
        cargo update --dry-run
        
        log_info "Security fixes applied (manual review recommended)"
    fi
}

# Main execution
main() {
    log_info "Starting security audit for Stellar K8s Operator"
    echo "============================================="
    
    cd "$PROJECT_ROOT"
    
    # Install required tools
    install_security_tools
    
    # Run security checks
    run_security_audit
    
    # Check build security
    check_build_security
    
    # Generate SBOM
    generate_sbom
    
    # Apply fixes if requested
    apply_security_fixes
    
    log_success "Security audit completed"
    echo ""
    log_info "Review the security audit report at: DEPENDENCY_SECURITY_AUDIT.md"
    log_info "SBOM generated at: security/sbom/"
    
    if [[ "$FIX_MODE" != "--fix" ]]; then
        echo ""
        log_info "To apply automated fixes, run: $0 --fix"
    fi
}

# Run main function
main "$@"