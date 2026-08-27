# Database Migration Testing

This document covers operator-owned SQL migrations, the automated harness, and
the review checklist for schema changes.

Horizon still owns its application schema. The operator applies Horizon upgrades
with `horizon db upgrade || horizon db init` via the `horizon-db-migration` init
container (`src/controller/resources.rs`). The SQL in `db/migrations/` records
and verifies those runs — it does **not** re-implement Horizon's schema.

## Migration technology

| Concern | Tool |
|---------|------|
| Operator-owned SQL | Versioned `db/migrations/*.up.sql` / `*.down.sql` files executed with **sqlx** (already a crate dependency) |
| Horizon application schema | Horizon CLI (`horizon db upgrade`) |
| Kubernetes CRD evolution | `scripts/crd_migration_lint.py` (separate from this harness) |

Do not introduce Flyway, Diesel, or sqlx-migrate. The harness uses `sqlx::query`
so up/down scripts stay plain SQL.

## Naming conventions

Files must match:

```text
db/migrations/NNNN_short_name.up.sql
db/migrations/NNNN_short_name.down.sql
```

- `NNNN` is a zero-padded integer, monotonically increasing.
- `short_name` is snake_case and describes the change.
- Every up file **must** have a matching down file.

## Forward migrations

- Prefer additive changes (new tables, columns, indexes).
- Use `IF NOT EXISTS` / `IF EXISTS` so re-runs in isolated schemas are safe.
- Do not put secrets or production connection strings in SQL.
- Keep statements simple; the runner splits on `;` and skips `--` comments.

## Rollback migrations

- Down scripts must restore a schema the previous operator version can read.
- Drop objects created by the matching up script only.
- Do not attempt to reconstruct destroyed production data. If a change is
  destructive, say so in the PR and in the up-file comment.

## Idempotency

Up scripts should be safe to apply against an empty schema. The harness always
runs in a temporary schema (`migtest_*`) that is dropped afterwards.

## Data migrations

When a migration transforms existing rows (for example `0002_add_run_checksum`
backfills `checksum`):

1. Capture representative rows **before** the change.
2. Assert primary keys still exist afterwards.
3. Assert the transformed column has the expected shape (`NOT NULL`, hex digest).

## Destructive changes

Avoid `DROP COLUMN` / `DROP TABLE` in up migrations unless a major version bump
is documented. If unavoidable:

- Call out the incompatibility in the PR.
- Provide a down script that cannot claim to restore deleted rows.

## Backward compatibility

Operator versions that still write `horizon_migration_runs` without `checksum`
must keep working until the down path has been exercised. New NOT NULL columns
must be backfilled in the same up script that adds them.

## Running the tests

```bash
# Local Postgres (safe test credentials only)
export DATABASE_URL=postgres://stellar:stellar_test@127.0.0.1:5432/stellar_migration_test

make test-db-migrations
# or
bash scripts/ci/test-db-migrations.sh
# or
cargo test --test db_migration_harness -- --nocapture
```

The harness:

1. Creates a temporary schema.
2. Runs **fresh** (all ups → schema checks → all downs → re-apply).
3. Runs **existing-data** (v1 → seed → remaining ups → integrity → rollback → re-apply).
4. Drops the temporary schema.

Without `DATABASE_URL`, local unit tests print a skip message. In CI
(`CI=true`) a missing URL is a hard failure.

## Review checklist

- [ ] Paired `NNNN_name.up.sql` and `NNNN_name.down.sql`
- [ ] Forward migration applies on an empty database
- [ ] Forward migration applies on a database that already has representative rows
- [ ] Down migration restores the previous compatible schema
- [ ] Integrity assertions cover row counts, primary keys, and transformed columns
- [ ] No production credentials in SQL or CI config
- [ ] `make test-db-migrations` (or the CI job) is green
