/// Deterministic test fixtures for reproducible integration tests.
///
/// Provides seeded RNG, fixed timestamps, and helper functions for generating
/// deterministic test data. Use these instead of `rand::thread_rng()`,
/// `Utc::now()`, or ad-hoc string literals to avoid flaky time- and
/// randomness-dependent tests.
use rand::SeedableRng;

/// Deterministic RNG for tests. Use `SeededRng::seeded(seed)` for reproducible
/// results.
pub struct SeededRng(rand::rngs::SmallRng);

impl SeededRng {
    pub fn seeded(seed: u64) -> Self {
        Self(rand::rngs::SmallRng::seed_from_u64(seed))
    }

    pub fn inner(&mut self) -> &mut rand::rngs::SmallRng {
        &mut self.0
    }
}

/// Fixed "now" timestamp for deterministic time-sensitive tests.
///
/// Returns 2026-01-15T12:00:00Z. Use this instead of `Utc::now()` in tests
/// to avoid flaky time-dependent assertions.
pub fn fixed_now() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-01-15T12:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc)
}

/// Generate a deterministic test namespace name from a seed.
pub fn test_namespace(seed: &str) -> String {
    format!("test-{}", seed)
}

/// Generate a deterministic test StellarNode name from a seed.
pub fn test_node_name(seed: &str) -> String {
    format!("node-{}", seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_rng_deterministic() {
        let mut a = SeededRng::seeded(42);
        let mut b = SeededRng::seeded(42);
        let va: u64 = rand::Rng::gen(a.inner());
        let vb: u64 = rand::Rng::gen(b.inner());
        assert_eq!(va, vb);
    }

    #[test]
    fn seeded_rng_different_seeds() {
        let mut a = SeededRng::seeded(1);
        let mut b = SeededRng::seeded(2);
        let va: u64 = rand::Rng::gen(a.inner());
        let vb: u64 = rand::Rng::gen(b.inner());
        assert_ne!(va, vb);
    }

    #[test]
    fn fixed_now_is_deterministic() {
        let t1 = fixed_now();
        let t2 = fixed_now();
        assert_eq!(t1, t2);
        assert_eq!(t1.to_rfc3339(), "2026-01-15T12:00:00+00:00");
    }

    #[test]
    fn test_namespace_format() {
        assert_eq!(test_namespace("mytest"), "test-mytest");
    }

    #[test]
    fn test_node_name_format() {
        assert_eq!(test_node_name("alpha"), "node-alpha");
    }
}
