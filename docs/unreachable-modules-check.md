# Unreachable Modules & Dead Code Path Checks

Static reachability audit for Rust sources under `src/`. Introduced for
[issue #1150](https://github.com/OtowoOrg/Stellar-K8s/issues/1150).

## What it checks

1. **Orphan source files** — `.rs` files that are never reached via `mod`
   declarations from any crate root (`src/lib.rs`, `src/main.rs`, and every
   `[[bin]]` path in `Cargo.toml`).
2. **Ambiguous module paths** — directories that contain both `foo.rs` and
   `foo/mod.rs` (rustc E0761).
3. **Dead code-path markers** — `todo!()`, `unimplemented!()`, and
   `unreachable!()` outside of string literals / `#[cfg(test)]` contexts.
   These are informational by default; pass `--strict-dead-paths` to fail on
   unfinished macros.

## Allowlist

Known WIP orphans are listed in
[`config/unreachable-modules-allowlist.txt`](../config/unreachable-modules-allowlist.txt).
Allowlisted orphans are reported but do not fail CI. New orphans that are
not on the allowlist fail the check so accidental dead modules cannot land
unnoticed.

Prefer wiring a module into `lib.rs` / a `[[bin]]` entry, or deleting it,
over growing the allowlist.

## Running locally

```bash
# Makefile entrypoint (same as CI)
make check-unreachable-modules

# Shell wrapper
./scripts/check-unreachable-modules.sh

# Direct binary
cargo run --locked --bin check-unreachable-modules

# Report-only (always exit 0)
./scripts/check-unreachable-modules.sh --report
```

## Unit tests

```bash
cargo test --locked --bin check-unreachable-modules
```

## CI

The `unreachable-modules` job in `.github/workflows/ci.yml` runs when
`rust_core` changes. It executes the shell wrapper and the binary's unit
tests.

## Interpreting failures

| Finding | Action |
|---------|--------|
| New orphan file | Wire it with `mod` / `pub mod`, add a `[[bin]]` entry, delete it, or add it to the allowlist with justification |
| Ambiguous path | Keep either `foo.rs` or `foo/mod.rs`, not both |
| Dead-path marker | Replace with a real implementation, or gate behind `#[cfg(test)]` |
