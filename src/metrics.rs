//! Metrics collection: cycles, cache hit/miss, memory stalls, slowdown.

use crate::core::CoreId;
use std::collections::HashMap;

/// Per-core and aggregate metrics.
#[derive(Clone, Default, Debug)]
pub struct Metrics {
    /// Total simulation cycles.
    pub total_cycles: u64,
    /// Total memory accesses (loads + stores).
    pub total_memory_accesses: u64,
    /// Cache hits.
    pub cache_hits: u64,
    /// Cache misses.
    pub cache_misses: u64,
    /// Cycles spent stalled on memory (cache miss penalty).
    pub memory_stall_cycles: u64,
    /// Per-core breakdown (optional).
    pub per_core: HashMap<CoreId, PerCoreMetrics>,
}

#[derive(Clone, Default, Debug)]
pub struct PerCoreMetrics {
    pub memory_accesses: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub memory_stall_cycles: u64,
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_access(&mut self, core_id: CoreId, hit: bool, stall_cycles: u64) {
        self.total_memory_accesses += 1;
        if hit {
            self.cache_hits += 1;
        } else {
            self.cache_misses += 1;
        }
        self.memory_stall_cycles += stall_cycles;
        let per = self.per_core.entry(core_id).or_default();
        per.memory_accesses += 1;
        if hit {
            per.cache_hits += 1;
        } else {
            per.cache_misses += 1;
        }
        per.memory_stall_cycles += stall_cycles;
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 1.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn miss_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_misses as f64 / total as f64
    }

    /// Slowdown = (actual_cycles - ideal_cycles) / ideal_cycles, or 0 if ideal is 0.
    pub fn slowdown_vs_ideal(&self, ideal_cycles: u64) -> f64 {
        if ideal_cycles == 0 {
            return 0.0;
        }
        let actual = self.total_cycles;
        if actual <= ideal_cycles {
            return 0.0;
        }
        (actual - ideal_cycles) as f64 / ideal_cycles as f64
    }

    /// Percentage slowdown (0..100+).
    pub fn slowdown_percent(&self, ideal_cycles: u64) -> f64 {
        self.slowdown_vs_ideal(ideal_cycles) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_hit_rate_no_accesses() {
        let m = Metrics::new();
        assert_eq!(m.hit_rate(), 1.0);
        assert_eq!(m.miss_rate(), 0.0);
    }

    #[test]
    fn metrics_hit_miss_rates() {
        let mut m = Metrics::new();
        m.record_access(CoreId(0), true, 0);
        m.record_access(CoreId(0), true, 0);
        m.record_access(CoreId(0), false, 100);
        assert_eq!(m.total_memory_accesses, 3);
        assert!((m.hit_rate() - 2.0 / 3.0).abs() < 1e-9);
        assert!((m.miss_rate() - 1.0 / 3.0).abs() < 1e-9);
        assert_eq!(m.memory_stall_cycles, 100);
    }

    #[test]
    fn metrics_slowdown() {
        let mut m = Metrics::new();
        m.total_cycles = 117;
        let ideal = 100;
        assert!((m.slowdown_percent(ideal) - 17.0).abs() < 0.01);
    }
}
