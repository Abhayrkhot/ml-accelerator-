//! L1 cache model: set-associative with configurable size, line size, and LRU replacement.

use crate::core::Cycle;
use std::collections::VecDeque;

/// Result of a cache access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheAccessResult {
    Hit,
    Miss,
}

/// Configuration for an L1 cache.
#[derive(Clone, Debug)]
pub struct CacheConfig {
    /// Total cache size in bytes.
    pub size_bytes: usize,
    /// Line size in bytes.
    pub line_size: usize,
    /// Associativity (number of ways per set).
    pub associativity: usize,
    /// Latency in cycles for a hit.
    pub hit_latency_cycles: u32,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            size_bytes: 4096,
            line_size: 64,
            associativity: 2,
            hit_latency_cycles: 1,
        }
    }
}

impl CacheConfig {
    pub fn num_sets(&self) -> usize {
        (self.size_bytes / self.line_size) / self.associativity
    }
}

/// One cache line (tag + optional LRU ordering).
#[derive(Clone, Debug)]
struct CacheLine {
    tag: u64,
    valid: bool,
}

/// One set: multiple ways with LRU ordering (index 0 = MRU, last = LRU).
struct CacheSet {
    lines: Vec<CacheLine>,
    /// FIFO/LRU order: front = most recently used, back = least recently used.
    lru_order: VecDeque<usize>,
}

impl CacheSet {
    fn new(associativity: usize) -> Self {
        let lines = (0..associativity)
            .map(|_| CacheLine {
                tag: 0,
                valid: false,
            })
            .collect();
        let lru_order = (0..associativity).collect();
        Self { lines, lru_order }
    }

    fn access(&mut self, tag: u64) -> CacheAccessResult {
        for (i, line) in self.lines.iter().enumerate() {
            if line.valid && line.tag == tag {
                self.touch(i);
                return CacheAccessResult::Hit;
            }
        }
        CacheAccessResult::Miss
    }

    fn allocate(&mut self, tag: u64) {
        if let Some(&victim_way) = self.lru_order.back() {
            self.lines[victim_way].tag = tag;
            self.lines[victim_way].valid = true;
            self.touch(victim_way);
        }
    }

    fn touch(&mut self, way: usize) {
        if let Some(pos) = self.lru_order.iter().position(|&w| w == way) {
            self.lru_order.remove(pos);
            self.lru_order.push_front(way);
        }
    }
}

/// Private L1 cache for one core.
pub struct Cache {
    config: CacheConfig,
    sets: Vec<CacheSet>,
    /// Mask to derive set index from address (after removing line offset bits).
    set_mask: u64,
    /// Number of bits for line offset (log2(line_size)).
    line_bits: u32,
}

impl Cache {
    pub fn new(config: CacheConfig) -> Self {
        let num_sets = config.num_sets();
        assert!(num_sets > 0, "cache must have at least one set");
        let sets = (0..num_sets)
            .map(|_| CacheSet::new(config.associativity))
            .collect();
        let line_bits = config.line_size.trailing_zeros();
        let set_bits = (num_sets as u64).trailing_zeros();
        let set_mask = (1u64 << set_bits) - 1;
        Self {
            config,
            sets,
            set_mask,
            line_bits,
        }
    }

    /// Returns (set_index, tag) for the given address.
    fn address_to_set_and_tag(&self, address: u64) -> (usize, u64) {
        let line_addr = address >> self.line_bits;
        let set_index = (line_addr & self.set_mask) as usize;
        let tag = line_addr >> (self.set_mask.count_ones());
        (set_index, tag)
    }

    /// Access the cache (read or write). Returns Hit or Miss.
    /// On miss, the line is allocated (after victim is evicted in real HW; we model that as allocation).
    pub fn access(&mut self, address: u64) -> CacheAccessResult {
        let (set_idx, tag) = self.address_to_set_and_tag(address);
        let set = &mut self.sets[set_idx];
        let result = set.access(tag);
        if result == CacheAccessResult::Miss {
            set.allocate(tag);
        }
        result
    }

    pub fn hit_latency_cycles(&self) -> u32 {
        self.config.hit_latency_cycles
    }

    pub fn config(&self) -> &CacheConfig {
        &self.config
    }

    /// Number of sets (for conflict pattern: addresses mapping to same set cause conflicts).
    pub fn num_sets(&self) -> usize {
        self.sets.len()
    }

    /// Line size in bytes (stride that maps to same set = size_bytes/num_sets).
    pub fn line_size(&self) -> usize {
        self.config.line_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_config_num_sets() {
        let c = CacheConfig {
            size_bytes: 256,
            line_size: 32,
            associativity: 2,
            hit_latency_cycles: 1,
        };
        assert_eq!(c.num_sets(), 4);
    }

    #[test]
    fn cache_hit_after_fill() {
        let config = CacheConfig {
            size_bytes: 256,
            line_size: 64,
            associativity: 2,
            hit_latency_cycles: 1,
        };
        let mut cache = Cache::new(config);
        let addr = 0u64;
        assert_eq!(cache.access(addr), CacheAccessResult::Miss);
        assert_eq!(cache.access(addr), CacheAccessResult::Hit);
    }

    #[test]
    fn cache_conflict_same_set() {
        // Direct-mapped (1 way), 4 sets: set_index = line_addr % 4.
        let config = CacheConfig {
            size_bytes: 128,
            line_size: 32,
            associativity: 1,
            hit_latency_cycles: 1,
        };
        let mut cache = Cache::new(config);
        let addr0 = 0u64;      // line_addr 0 -> set 0
        let addr1 = 128u64;    // line_addr 4 -> set 0 (evicts addr0)
        cache.access(addr0);
        cache.access(addr1);
        assert_eq!(cache.access(addr0), CacheAccessResult::Miss);
    }

    #[test]
    fn cache_different_sets_hit() {
        let config = CacheConfig {
            size_bytes: 256,
            line_size: 64,
            associativity: 2,
            hit_latency_cycles: 1,
        };
        let mut cache = Cache::new(config);
        // 4 sets. Addresses 0, 256, 512, ... map to different sets.
        cache.access(0);
        cache.access(256);
        assert_eq!(cache.access(0), CacheAccessResult::Hit);
        assert_eq!(cache.access(256), CacheAccessResult::Hit);
    }
}
