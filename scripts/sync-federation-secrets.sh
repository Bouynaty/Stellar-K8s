#!/bin/bash
# Federation Secret & Config Synchronisation
#
# #1409 — Cross-cluster secret and configuration synchronization.
#
# Copies a Secret (typically a kubeconfig for the federation controller, or a
# Stellar validator seed needed at failover time) from a source cluster context
# to a target cluster context, creating or updating it idempotently.
#
# Usage:
#   ./scripts/sync-federation-secrets.sh \
#     --source-cluster us-east-1 \
#     --target-cluster eu-west-1 \
#     --name stellar-federation-us-east-1 \
#     --namespace stellar-system \
#     [--dry-run]
#
# Environment variables mirror the flags (SOURCE_CLUSTER, TARGET_CLUSTER, ...).

set -euo pipefail

SOURCE_CLUSTER="${SOURCE_CLUSTER:-}"
TARGET_CLUSTER="${TARGET_CLUSTER:-}"
SECRET_NAME="${SECRET_NAME:-}"
NAMESPACE="${NAMESPACE:-stellar-system}"
DRY_RUN="${DRY_RUN:-false}"

usage() {
    cat <<EOF
usage: $0 --source-cluster <ctx> --target-cluster <ctx> --name <secret> [--namespace <ns>] [--dry-run]
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --source-cluster) SOURCE_CLUSTER="$2"; shift 2 ;;
        --target-cluster) TARGET_CLUSTER="$2"; shift 2 ;;
        --name) SECRET_NAME="$2"; shift 2 ;;
        --namespace) NAMESPACE="$2"; shift 2 ;;
        --dry-run) DRY_RUN=true; shift ;;
        *) shift ;;
    esac
done

[[ -n "$SOURCE_CLUSTER" && -n "$TARGET_CLUSTER" && -n "$SECRET_NAME" ]] || usage

if [[ "$DRY_RUN" == "true" ]]; then
    echo "[dry-run] would copy secret '$SECRET_NAME' from '$SOURCE_CLUSTER' to '$TARGET_CLUSTER/$NAMESPACE'"
    exit 0
fi

# Render the source secret through a client-side dry-run apply to strip
# server-managed fields (metadata.generation, resourceVersion, uid, ...).
SECRET_YAML=$(
    kubectl --context "$SOURCE_CLUSTER" -n "$NAMESPACE" get secret "$SECRET_NAME" -o yaml \
        | kubectl apply --dry-run=client -f - -o yaml -
)
if [[ -z "$SECRET_YAML" ]]; then
    echo "error: secret '$SECRET_NAME' not found in '$SOURCE_CLUSTER/$NAMESPACE'" >&2
    exit 1
fi

echo "→ Syncing secret '$SECRET_NAME' → '$TARGET_CLUSTER/$NAMESPACE'"
printf '%s' "$SECRET_YAML" \
    | kubectl --context "$TARGET_CLUSTER" apply -f - --validate=false
echo "✓ Secret '$SECRET_NAME' synced to '$TARGET_CLUSTER/$NAMESPACE'"