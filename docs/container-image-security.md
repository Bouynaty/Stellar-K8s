# Container Image Security

Issue #1334. Covers image vulnerability scanning and Cosign supply-chain
signing.

## What changed

Trivy already scanned images and uploaded SARIF to the Security tab, but the
action never set `exit-code`, so **a critical CVE could not fail anything**.
Nothing was signed, and pull requests got no image scan at all — the `docker`
job in `ci.yml` is gated on `github.event_name != 'pull_request'`.

| Gate | Where | Blocks |
|---|---|---|
| PR image scan | `ci.yml` → `image-scan` | Merge |
| Release image scan | `release.yml` → `security` | The GitHub release |
| Cosign signing | `release.yml` → `container` | — |
| Signature verification | `release.yml` → `provenance-check` | The release being trusted |

## Vulnerability gate

[`.github/actions/security-scan`](https://github.com/OtowoOrg/Stellar-K8s/blob/main/.github/actions/security-scan/action.yml)
now runs two passes:

1. **Report** — full `CRITICAL,HIGH` range, SARIF uploaded to the Security tab.
2. **Gate** — `fail-on-severity` (default `CRITICAL`) with `exit-code: 1`.

They are separate passes so the SARIF upload still happens for the full
severity range even when the gate fails the job.

`ignore-unfixed` defaults to `true` in the gate: a base-image CVE with no
upstream fix should not wedge every merge in the repository, and it is still
reported in the Security tab by the first pass.

To make a caller report-only, pass `fail-on-severity: ""`.

### PR gate

`image-scan` builds the image locally with `push: false` — nothing from an
unreviewed pull request reaches the registry — and scans it. It runs only when
the `changes` job detects Docker-relevant edits, and reuses the shared
BuildKit cache.

### Release gate

`release.yml`'s `release` job already depends on `security`, so a critical
vulnerability now blocks the release itself, satisfying "block deployment of
images with critical vulnerabilities".

## Signing

[`.github/actions/sign-image`](https://github.com/OtowoOrg/Stellar-K8s/blob/main/.github/actions/sign-image/action.yml)
signs **keylessly** via Fulcio/Rekor, using the GitHub Actions OIDC token as
the signing identity. There is no private key to store, rotate, or leak. The
calling job needs `id-token: write` and `packages: write`.

Images are signed **by digest**, not by tag — a tag can later be moved to
different content, so a tag signature proves nothing about what you pull.

Signing complements, rather than replaces, the existing build provenance
attestation:

- **Provenance** says *how* the image was built.
- **Signature** says *this exact digest came from this repository*, and is what
  a cluster-side admission policy verifies before pulling.

### Verifying an image

```bash
cosign verify \
  --certificate-identity-regexp "^https://github.com/OtowoOrg/Stellar-K8s/" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  ghcr.io/otoworg/stellar-operator:0.1.0
```

The identity regexp matters: without it, `cosign verify` accepts a signature
from *any* Fulcio identity that happens to have signed the digest.

## Verification

Both actions are composite YAML, so the checks that can run locally are
structural:

```bash
# Both action definitions parse and declare the expected inputs
python3 - <<'PY'
import yaml
scan = yaml.safe_load(open(".github/actions/security-scan/action.yml"))
sign = yaml.safe_load(open(".github/actions/sign-image/action.yml"))
assert "fail-on-severity" in scan["inputs"]
assert scan["inputs"]["fail-on-severity"]["default"] == "CRITICAL"
assert {"image", "digest"} <= set(sign["inputs"])
print("action definitions OK")
PY

# The PR gate exists and does not push
python3 - <<'PY'
import yaml
ci = yaml.safe_load(open(".github/workflows/ci.yml"))
job = ci["jobs"]["image-scan"]
build = next(s for s in job["steps"] if "build-push-action" in str(s.get("uses")))
assert build["with"]["push"] is False, "PR builds must never push"
print("PR image gate OK")
PY
```

End-to-end, in CI:

1. Open a PR touching `Dockerfile` → the **Image Vulnerability Gate** job runs
   and fails if the image has a fixable critical CVE.
2. Push a `v*` tag → `container` signs the image, `security` scans it, and
   `provenance-check` verifies both the attestation and the Cosign signature.
3. `cosign verify` (above) against the published tag returns the certificate
   identity of the workflow that signed it.
