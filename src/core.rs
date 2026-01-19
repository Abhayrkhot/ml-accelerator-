//! Core architecture model: cycles, cores, pipeline stages, and instruction representation.

use std::fmt;

/// Global simulation cycle counter (discrete time).
pub type Cycle = u64;

/// Identifies a core (0..N).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoreId(pub usize);

/// Identifies a thread (for scheduling).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ThreadId(pub usize);

/// Pipeline stage for instruction-level parallelism modeling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineStage {
    Fetch,
    Execute,
    Memory,
    Commit,
}

/// Kind of operation an instruction performs (for latency modeling).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstructionKind {
    /// Compute (execute stage only).
    Compute,
    /// Load: may hit L1 or miss to memory.
    Load,
    /// Store: may hit L1 or miss to memory.
    Store,
}

/// A single instruction in the pipeline.
#[derive(Clone, Debug)]
pub struct Instruction {
    pub kind: InstructionKind,
    /// Logical address (used for cache indexing and memory).
    pub address: u64,
    /// Cycle when this instruction entered the pipeline.
    pub issue_cycle: Cycle,
    /// Cycles remaining in current stage (0 = ready to advance).
    pub stage_cycles_left: u32,
    /// Current pipeline stage.
    pub stage: PipelineStage,
    /// Whether this instruction is stalled (e.g. cache miss, structural hazard).
    pub stalled: bool,
    /// If stalled, cycles remaining until stall ends.
    pub stall_cycles_left: u32,
}

impl Instruction {
    pub fn new_compute(issue_cycle: Cycle) -> Self {
        Self {
            kind: InstructionKind::Compute,
            address: 0,
            issue_cycle,
            stage_cycles_left: 1,
            stage: PipelineStage::Fetch,
            stalled: false,
            stall_cycles_left: 0,
        }
    }

    pub fn new_memory(kind: InstructionKind, address: u64, issue_cycle: Cycle) -> Self {
        Self {
            kind,
            address,
            issue_cycle,
            stage_cycles_left: 1,
            stage: PipelineStage::Fetch,
            stalled: false,
            stall_cycles_left: 0,
        }
    }

    pub fn is_memory_op(&self) -> bool {
        matches!(self.kind, InstructionKind::Load | InstructionKind::Store)
    }
}

impl fmt::Display for PipelineStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipelineStage::Fetch => write!(f, "Fetch"),
            PipelineStage::Execute => write!(f, "Execute"),
            PipelineStage::Memory => write!(f, "Memory"),
            PipelineStage::Commit => write!(f, "Commit"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_compute_creation() {
        let i = Instruction::new_compute(0);
        assert!(!i.is_memory_op());
        assert_eq!(i.stage, PipelineStage::Fetch);
    }

    #[test]
    fn instruction_memory_creation() {
        let load = Instruction::new_memory(InstructionKind::Load, 0x1000, 0);
        let store = Instruction::new_memory(InstructionKind::Store, 0x2000, 0);
        assert!(load.is_memory_op());
        assert!(store.is_memory_op());
        assert_eq!(load.address, 0x1000);
    }
}
