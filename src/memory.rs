//! Shared memory with configurable access latency (modeling DRAM).

use crate::core::Cycle;

/// Configuration for shared memory.
#[derive(Clone, Debug)]
pub struct MemoryConfig {
    /// Latency in cycles for a memory access (miss penalty).
    pub access_latency_cycles: u32,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            access_latency_cycles: 100,
        }
    }
}

/// Shared memory subsystem. Models latency only (no actual data storage for the simulator).
pub struct Memory {
    config: MemoryConfig,
}

impl Memory {
    pub fn new(config: MemoryConfig) -> Self {
        Self { config }
    }

    /// Returns the number of cycles a memory access takes (stall duration).
    pub fn access_latency_cycles(&self) -> u32 {
        self.config.access_latency_cycles
    }

    pub fn config(&self) -> &MemoryConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_default_latency() {
        let mem = Memory::new(MemoryConfig::default());
        assert_eq!(mem.access_latency_cycles(), 100);
    }

    #[test]
    fn memory_custom_latency() {
        let mem = Memory::new(MemoryConfig {
            access_latency_cycles: 50,
        });
        assert_eq!(mem.access_latency_cycles(), 50);
    }
}
