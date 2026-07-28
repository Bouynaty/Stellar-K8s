## **CI Check Errors - Summary & Fixes**

The PR check failed on **cache key validation**. Your repo-hygiene job is correctly enforcing a strict cache key format, but several workflows are using non-compliant keys. Here are all 8 errors and how to fix them:

---

### **Invalid Cache Keys Found:**

| Error | Workflow | Current Key | Required Format | Fix |
|-------|----------|-------------|-----------------|-----|
| **#1** | `e2e-quickstart.yml` | `e2e-quickstart` | `<prefix>-<name>` | Change to `ci-e2e-quickstart` or `verify-quickstart` |
| **#2** | `dependency-review.yml` | `dep-audit` | `<prefix>-<name>` | Change to `ci-audit` |
| **#3** | `dependency-review.yml` | `dep-stale` | `<prefix>-<name>` | Change to `ci-stale` |
| **#4** | `dependency-review.yml` | `dep-diff` | `<prefix>-<name>` | Change to `ci-diff` |
| **#5** | `dependency-review.yml` | `dep-license` | `<prefix>-<name>` | Change to `ci-license` |
| **#6** | `release-gate.yml` or build | `release-${{matrix.target}}` | `<prefix>-<name>` | Change to `release-${{ matrix.target }}` (if it already has valid prefix) or use `build-${{ matrix.target }}` |
| **#7** | `maintenance.yml` | `maintenance` | `<prefix>-<name>` | Change to `ci-maintenance` |
| **#8** | `maintenance.yml` | `maintenance-audit` | `<prefix>-<name>` | Change to `ci-audit-scheduled` or similar |

---

### **Valid Cache Key Prefixes (from your check script):**
- `ci` — general CI/build tasks
- `perf` — performance testing
- `soak` — soak/stress testing
- `release` — release builds
- `chaos` — chaos engineering
- `docs` — documentation
- `verify` — verification tasks
- `image` — Docker image builds

---

### **Quick Fix List:**

#### **1. `.github/workflows/dependency-review.yml`** (Lines 127, 159, 191, 217)
```diff
- cache-key: "dep-audit"
+ cache-key: "ci-audit"

- cache-key: "dep-stale"
+ cache-key: "ci-stale"

- cache-key: "dep-diff"
+ cache-key: "ci-diff"

- cache-key: "dep-license"
+ cache-key: "ci-license"
```

#### **2. `.github/workflows/e2e-quickstart.yml`** (Line ~58)
```diff
- cache-key: "e2e-quickstart"
+ cache-key: "verify-quickstart"
```

#### **3. `.github/workflows/maintenance.yml`** (Lines 14, 33)
```diff
- cache-key: "maintenance"
+ cache-key: "ci-maintenance"

- cache-key: "maintenance-audit"
+ cache-key: "ci-audit-maintenance"
```

#### **4. `.github/workflows/release-gate.yml`** (check the cache-key parameter)
If it uses `release-${{matrix.target}}` without a matrix context, change to:
```diff
- cache-key: "release-${{matrix.target}}"
+ cache-key: "release-operator"  # or whatever the actual target is
```

---

### **Why This Matters:**
Your `scripts/ci/check-cache-keys.sh` script enforces this format to:
- ✅ Ensure predictable cache invalidation
- ✅ Prevent cache collisions between different job types
- ✅ Make cache strategy transparent and auditable

Once you fix these 8 cache keys, the repo-hygiene check will pass! 🎯
