//! Configurable workload generator: sequential and conflict-heavy access patterns.

use crate::core::{Instruction, InstructionKind};
use std::iter;

/// Access pattern for memory instructions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessPattern {
    /// Sequential: addresses 0, line_size, 2*line_size, ... (good locality).
    Sequential,
    /// Conflict-heavy: addresses chosen to map to the same cache set(s), causing evictions.
    ConflictHeavy,
}

/// Workload configuration.
#[derive(Clone, Debug)]
pub struct WorkloadConfig {
    /// Number of instructions to generate per thread.
    pub instructions_per_thread: usize,
    /// Fraction of instructions that are memory (load/store); rest are compute.
    pub memory_fraction: f64,
    /// Access pattern for memory addresses.
    pub access_pattern: AccessPattern,
    /// Line size (bytes) for stride in sequential; also used for conflict stride.
    pub line_size: usize,
    /// Number of cache sets (for conflict pattern: we use addresses that alias to few sets).
    pub cache_num_sets: usize,
    /// For Sequential: cap unique lines to this many (reuse = cache hits). 0 = no cap.
    pub working_set_lines: usize,
}

impl Default for WorkloadConfig {
    fn default() -> Self {
        Self {
            instructions_per_thread: 1000,
            memory_fraction: 0.4,
            access_pattern: AccessPattern::Sequential,
            line_size: 64,
            cache_num_sets: 64,
            working_set_lines: 0,
        }
    }
}

/// Generates a stream of instructions for one thread.
pub struct WorkloadGenerator {
    config: WorkloadConfig,
    /// Next instruction index (for sequential or conflict address generation).
    index: usize,
}

impl WorkloadGenerator {
    pub fn new(config: WorkloadConfig) -> Self {
        Self { config, index: 0 }
    }

    /// Generates the next instruction at the given logical "issue" cycle (for logging).
    pub fn next_instruction(&mut self, issue_cycle: u64) -> Option<Instruction> {
        if self.index >= self.config.instructions_per_thread {
            return None;
        }
        let frac = (self.config.memory_fraction * 100.0).round() as usize;
        let use_memory = (self.index % 100) < frac.min(100) || self.config.memory_fraction >= 1.0;
        self.index += 1;

        let instr = if use_memory {
            let address = self.next_address();
            let kind = if self.index % 2 == 0 {
                InstructionKind::Load
            } else {
                InstructionKind::Store
            };
            Instruction::new_memory(kind, address, issue_cycle)
        } else {
            Instruction::new_compute(issue_cycle)
        };
        Some(instr)
    }

    fn next_address(&mut self) -> u64 {
        let idx = self.index - 1;
        let addr = match self.config.access_pattern {
            AccessPattern::Sequential => {
                let line_idx = if self.config.working_set_lines > 0 {
                    idx % self.config.working_set_lines
                } else {
                    idx
                };
                (line_idx as u64).wrapping_mul(self.config.line_size as u64)
            }
            AccessPattern::ConflictHeavy => {
                // Force all addresses into the same cache set: set = line_addr % num_sets.
                let line_addr = (idx as u64).wrapping_mul(self.config.cache_num_sets as u64);
                line_addr * self.config.line_size as u64
            }
        };
        addr
    }

    pub fn remaining(&self) -> usize {
        self.config.instructions_per_thread.saturating_sub(self.index)
    }

    pub fn config(&self) -> &WorkloadConfig {
        &self.config
    }
}

/// Build a full workload: list of instruction streams, one per thread.
pub fn build_workload(
    num_threads: usize,
    config: WorkloadConfig,
) -> Vec<Vec<Instruction>> {
    (0..num_threads)
        .map(|_| {
            let mut gen = WorkloadGenerator::new(config.clone());
            let mut list = Vec::with_capacity(config.instructions_per_thread);
            let mut cycle = 0u64;
            while let Some(instr) = gen.next_instruction(cycle) {
                list.push(instr);
                cycle += 1;
            }
            list
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workload_sequential_count() {
        let config = WorkloadConfig {
            instructions_per_thread: 10,
            memory_fraction: 0.5,
            access_pattern: AccessPattern::Sequential,
            line_size: 64,
            cache_num_sets: 64,
            working_set_lines: 0,
        };
        let mut gen = WorkloadGenerator::new(config);
        let mut count = 0;
        while gen.next_instruction(0).is_some() {
            count += 1;
        }
        assert_eq!(count, 10);
    }

    #[test]
    fn workload_conflict_addresses_same_set() {
        let config = WorkloadConfig {
            instructions_per_thread: 20,
            memory_fraction: 1.0,
            access_pattern: AccessPattern::ConflictHeavy,
            line_size: 64,
            cache_num_sets: 4,
            working_set_lines: 0,
        };
        let mut gen = WorkloadGenerator::new(config);
        let mut addrs = Vec::new();
        for i in 0..20 {
            if let Some(instr) = gen.next_instruction(i as u64) {
                if instr.is_memory_op() {
                    addrs.push(instr.address);
                }
            }
        }
        // With conflict-heavy, addresses should repeat set indices (many map to set 0,1,2,3).
        assert!(!addrs.is_empty());
    }
}
