# Static Analysis Gates

Three repository-wide gates run in CI and are runnable locally. Each one
replaces or extends a check that was previously partial, advisory, or
structurally unable to fail.

| Gate | Command | Issue | Replaces |
|---|---|---|---|
| Shell safety | `make shell-safety` | #1049 | Extends `shellcheck -S error` |
| YAML manifest validation | `make validate-yaml` | #1044 | Extends `validate-config-samples.sh` |
| Helm template drift | `make helm-drift` | #1045 | Replaces `check-chart-diff.sh` |

All three need only Python 3 with `pyyaml` and `jsonschema`:

```bash
pip install pyyaml jsonschema
```

---

## Shell safety gate (#1049)

`scripts/check-shell-safety.py` scans shell scripts for patterns that cause
silent data loss, command injection, or non-deterministic CI behaviour.

It exists alongside `shellcheck`, not instead of it. The repository runs
`shellcheck -S error`, which by design reports only what a linter considers
fatal — it stays quiet about an unquoted `rm -rf $DIR`, a `curl | bash`, or a
script that never enabled `set -euo pipefail`. Those are stylistic to a
linter and operational to an operator repository.

### Running it

```bash
make shell-safety                              # gate the repo (scripts/)
python3 scripts/check-shell-safety.py --strict # warnings fail too
python3 scripts/check-shell-safety.py --list-rules
python3 scripts/check-shell-safety.py --format json scripts/preflight.sh
make test-shell-safety                         # the gate's own unit tests
```

Exit codes: `0` pass, `1` findings at `error` severity, `2` bad invocation.

### Rules

| ID | Severity | Detects |
|---|---|---|
| `SH000` | error | A suppression pragma with no `--` reason |
| `SH001` | error | Executable script without `set -euo pipefail` |
| `SH002` | error | Unquoted expansion passed to `rm`/`mv`/`chmod`/`dd`/… |
| `SH003` | error | `rm -rf "$dir"/sub` with no empty-value guard |
| `SH004` | error | `eval` on interpolated data |
| `SH005` | error | `curl … \| bash` |
| `SH006` | error | `-k` / `--insecure` / `--no-check-certificate` |
| `SH007` | error | `chmod 777`, `a+rwx`, `o+w` |
| `SH008` | warning | Predictable `/tmp/name` instead of `mktemp` |
| `SH009` | warning | `mktemp` result with no cleanup |
| `SH010` | error | `cd` whose failure is unhandled (non-strict scripts only) |
| `SH011` | warning | Backtick command substitution |
| `SH012` | error | Unquoted `$@` / `$*` argument forwarding |
| `SH013` | warning | Iterating over `ls`/`find` output |
| `SH014` | error | Unquoted expansion inside `[ … ]` |
| `SH015` | error | `curl` without `-f`/`--fail` |

The checker understands shell context: comments, heredoc bodies, and
single-quoted spans are inert; `#` inside a string does not start a comment;
`"$*"` inside a quoted message is not flagged while a bare `$@` is.

Some rules are deliberately narrower than they first appear, because a rule
that fires on safe code is a rule people learn to ignore:

- **`SH003`** flags `rm -rf "$dir"/build` (an empty `$dir` deletes `/build`)
  but not a bare `rm -rf "$dir"` (`rm -rf ""` is a harmless no-op).
- **`SH010`** is silent under `set -e`, which already aborts on a failed `cd`.
- **`SH001`** exempts sourced libraries (`scripts/lib/*`, files with no
  shebang) and `.bats` files, which must not impose `set -e` on their caller.

### Waiving a finding

Every waiver needs a `--` reason; a bare `allow` is itself an error, so
waivers cannot be added silently.

```bash
rm -rf "$BUILD_DIR"/artifacts  # shell-safety: allow SH003 -- path validated above

# shell-safety: allow SH002 -- fixture path is a literal
rm -rf $FIXTURE

# In the file header, for a file-wide rule:
# shell-safety: disable-file SH001 -- report must exit 0 even when cargo fails
```

Repository-wide exclusions and severity overrides live in
`config/shell-safety.yaml`. Prefer an inline pragma: it keeps the
justification next to the line that needs it.

### Current state

The repository is clean at `error` severity. Four `SH008` warnings remain
(predictable `/tmp` paths in CI helper scripts) and are visible in every run.

---

## YAML manifest validation (#1044)

`scripts/validate-yaml-manifests.py` validates **every** YAML file in the
repository in four layers.

The previous check (`scripts/ci/validate-config-samples.sh`, still run) looked
at `examples/` and `config/samples/` only, delegated to `kubeconform` with
`-ignore-missing-schemas` — so every `stellar.org` custom resource was skipped
outright — and downgraded all findings to warnings.

| Layer | What it checks |
|---|---|
| `L1-syntax` | Every document parses; duplicate mapping keys and literal tabs are errors |
| `L2-structure` | Kubernetes docs have a well-formed `apiVersion`/`kind`, a DNS-1123 `metadata.name`, and valid label/annotation keys and values |
| `L3-schema` | Custom resources validate against this repo's own CRDs in `config/crd/`; any path can be bound to a JSON Schema |
| `L4-fixture` | Manifests declared as negative fixtures **must** fail — a schema that silently starts accepting bad input is caught |

Duplicate keys matter because PyYAML (and most YAML loaders) silently keep the
last value: that is how a manifest ends up with two `image:` keys and quietly
deploys the wrong one.

### Running it

```bash
make validate-yaml
python3 scripts/validate-yaml-manifests.py --summary
python3 scripts/validate-yaml-manifests.py --format json
python3 scripts/validate-yaml-manifests.py examples/          # a subtree
make test-yaml-validation                                     # unit tests
```

### Configuration

`config/yaml-validation.yaml` holds four lists:

- **`exclude`** — files not read at all (Helm templates are Go templates, not
  YAML; their rendered output is covered by the drift gate below).
- **`schemas`** — bind a path glob to a JSON Schema, e.g. the chart's
  `values.yaml` to `values.schema.json`.
- **`expect_invalid`** — manifests that must fail, such as
  `config/samples/invalid-*.yaml`.
- **`known_deviations`** — pre-existing failures reported as warnings instead
  of errors. Every entry needs a reason, **and a waiver that stops matching
  anything is reported as an error**, so the list cannot silently rot.

### Current state

Zero errors. 233 warnings, all recorded in `known_deviations`:

- **212** come from one root cause. `src/crd/schema_utils.rs` supplies
  hand-written schemas via `#[schemars(schema_with = ...)]`. schemars applies
  that override *before* it unwraps `Option<T>`, so `minAvailable`,
  `maxUnavailable`, and `topologySpreadConstraints` — all `Option<...>` in
  `src/crd/stellar_node.rs` — land in the generated CRD's `required` list.
  The operator treats them as optional; only the generated CRD disagrees.
  Fixing it means changing CRD generation and regenerating `config/crd/`.
- The rest are illustrative examples that intentionally omit unrelated
  required fields.

---

## Helm template drift detection (#1045)

`scripts/check-helm-drift.sh` renders the chart across five value profiles,
normalises each render through `scripts/sort-manifests.py`, and diffs the
result against golden files committed under
`charts/stellar-operator/rendered/`.

The predecessor, `scripts/check-chart-diff.sh`, compared against a baseline in
gitignored `.cache/`. In CI that directory is always empty, so the baseline was
recreated on every run and the comparison never happened — drift detection
that structurally could not fail. (That script was also committed twice into
the same file.) It has been removed.

Storing goldens in git means a template change that alters rendered output
shows up as a concrete manifest diff in the pull request, reviewable like any
other file.

### Profiles

| Profile | Values |
|---|---|
| `default` | `values.yaml` |
| `ha` | `values-ha.yaml` |
| `production` | `examples/values-production.yaml` |
| `development` | `examples/values-development.yaml` |
| `dr-cross-region` | DR + cross-region bridge enabled with one peer cluster |

### Running it

```bash
make helm-drift                                  # verify
make helm-drift-update                           # regenerate goldens
scripts/check-helm-drift.sh --profile production # one profile
scripts/check-helm-drift.sh --list
make test-helm-drift                             # bats tests
```

`make helm-lint` runs the drift check too.

### Intentional template changes

```bash
make helm-drift-update
git diff charts/stellar-operator/rendered   # review the rendered impact
git add charts/stellar-operator/rendered
```

Reviewing that diff is the point: it shows exactly which manifests a template
edit changes.

### What it caught

`charts/stellar-operator/examples/values-production.yaml` did not render at
all. `templates/cross-region-bridge.yaml` dereferenced `.Values.crossRegion`,
a key absent from every values file, so `featureFlags.enableDr: true` alone
aborted rendering with a nil-pointer error. The template is now nil-safe and
`values.yaml` documents a `crossRegion` block. `scripts/tests/helm-drift.bats`
carries regression tests for it.

---

## Verification

```bash
make shell-safety && make test-shell-safety
make validate-yaml && make test-yaml-validation
make helm-drift && make test-helm-drift
```

All three gates should report zero errors on a clean checkout.
