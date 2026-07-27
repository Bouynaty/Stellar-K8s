#!/usr/bin/env bash
# scripts/quickstart-verify.sh — End-to-end quickstart verification for Stellar-K8s.
# Forwarding script for scripts/archive/quickstart-verify.sh.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec bash "${SCRIPT_DIR}/archive/quickstart-verify.sh" "$@"
