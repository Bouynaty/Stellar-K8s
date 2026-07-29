#!/usr/bin/env bash
# check-pipeline-log-redaction.sh
#
# Enforce secret redaction checks in logs produced by pipeline commands.
# Closes Issue #1153.
#
# Wraps the `check-pipeline-log-redaction` Cargo binary so Makefile and CI share
# a single entrypoint. See docs/log-redaction-policy.md (§ Pipeline log checks).
#
# Exit codes
# ----------
#   0  — No findings (or --report)
#   1  — One or more redaction failures
#   2  — Tooling / usage error
#
# Usage:
#   ./scripts/check-pipeline-log-redaction.sh
#   ./scripts/check-pipeline-log-redaction.sh --report
#   ./scripts/check-pipeline-log-redaction.sh --fixture path/to/log.txt
#   ./scripts/check-pipeline-log-redaction.sh --scrub path/to/job.log

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "→ Checking pipeline log secret redaction..."
exec cargo run --quiet --locked --bin check-pipeline-log-redaction -- "$@"
