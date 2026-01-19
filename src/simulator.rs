//! Event-driven multicore simulator: cycle stepping, pipeline, cache/memory, metrics.

use crate::cache::{Cache, CacheAccessResult, CacheConfig};
use crate::core::{CoreId, Cycle, Instruction, InstructionKind, PipelineStage, ThreadId};
use crate::memory::{Memory, MemoryConfig};
use crate::metrics::Metrics;
use crate::scheduler::Scheduler;
use std::collections::VecDeque;

/// Per-core state: L1 cache, pipeline (in-flight instructions), and workload queue.
struct CoreState {
    cache: Cache,
    /// Instructions in pipeline (fetch -> execute -> memory -> commit).
    pipeline: VecDeque<Instruction>,
    /// Pending workload (instructions not yet fetched).
    workload: VecDeque<Instruction>,
    /// Max pipeline width (instructions in flight per core).
    pipeline_width: usize,
}

/// Event-driven multicore simulator.
pub struct Simulator {
    num_cores: usize,
    num_threads: usize,
    cores: Vec<CoreState>,
    memory: Memory,
    scheduler: Scheduler,
    pub metrics: Metrics,
    current_cycle: Cycle,
    /// Cycles per pipeline stage (fetch=1, execute=1, memory=1 or hit/miss, commit=1).
    stage_cycles: StageCycles,
}

#[derive(Clone)]
pub struct StageCycles {
    pub fetch_cycles: u32,
    pub execute_cycles: u32,
    pub commit_cycles: u32,
}

impl Default for StageCycles {
    fn default() -> Self {
        Self {
            fetch_cycles: 1,
            execute_cycles: 1,
            commit_cycles: 1,
        }
    }
}

impl Simulator {
    pub fn new(
        num_cores: usize,
        num_threads: usize,
        cache_config: CacheConfig,
        memory_config: MemoryConfig,
        pipeline_width: usize,
    ) -> Self {
        let cores = (0..num_cores)
            .map(|_| CoreState {
                cache: Cache::new(cache_config.clone()),
                pipeline: VecDeque::new(),
                workload: VecDeque::new(),
                pipeline_width,
            })
            .collect();
        let scheduler = Scheduler::new(num_cores, num_threads);
        let mut sim = Self {
            num_cores,
            num_threads,
            cores,
            memory: Memory::new(memory_config),
            scheduler,
            metrics: Metrics::new(),
            current_cycle: 0,
            stage_cycles: StageCycles::default(),
        };
        sim.metrics.total_cycles = 0;
        sim
    }

    /// Load workload per thread: thread_workloads[thread_id] = list of instructions.
    pub fn load_workload(&mut self, thread_workloads: Vec<Vec<Instruction>>) {
        for (thread_id, instrs) in thread_workloads.into_iter().enumerate() {
            let core_id = self.scheduler.thread_to_core(ThreadId(thread_id));
            for i in instrs {
                self.cores[core_id.0].workload.push_back(i);
            }
        }
    }

    /// Run one cycle of the event-driven simulation.
    pub fn step(&mut self) {
        self.current_cycle += 1;

        // 1) Commit stage: drain completed instructions.
        for core_id in 0..self.num_cores {
            let core = &mut self.cores[core_id];
            let mut i = 0;
            while i < core.pipeline.len() {
                let instr = &mut core.pipeline[i];
                if instr.stage != PipelineStage::Commit {
                    i += 1;
                    continue;
                }
                if instr.stage_cycles_left > 0 {
                    instr.stage_cycles_left -= 1;
                    i += 1;
                    continue;
                }
                // Remove from pipeline.
                core.pipeline.remove(i);
                continue;
            }
        }

        // 2) Memory stage: advance or stall.
        for core_id in 0..self.num_cores {
            let core = &mut self.cores[core_id];
            for instr in core.pipeline.iter_mut() {
                if instr.stage != PipelineStage::Memory {
                    continue;
                }
                if instr.stalled {
                    if instr.stall_cycles_left > 0 {
                        instr.stall_cycles_left -= 1;
                        self.metrics.memory_stall_cycles += 1;
                        let per = self.metrics.per_core.entry(CoreId(core_id)).or_default();
                        per.memory_stall_cycles += 1;
                    }
                    if instr.stall_cycles_left == 0 {
                        instr.stalled = false;
                        instr.stage_cycles_left = core.cache.hit_latency_cycles();
                    }
                    continue;
                }
                if instr.stage_cycles_left > 0 {
                    instr.stage_cycles_left -= 1;
                    continue;
                }
                // Memory stage done -> go to commit.
                instr.stage = PipelineStage::Commit;
                instr.stage_cycles_left = self.stage_cycles.commit_cycles;
            }
        }

        // 3) Execute stage: advance; memory ops go to Memory stage and trigger cache access.
        for core_id in 0..self.num_cores {
            let core = &mut self.cores[core_id];
            for instr in core.pipeline.iter_mut() {
                if instr.stage != PipelineStage::Execute {
                    continue;
                }
                if instr.stage_cycles_left > 0 {
                    instr.stage_cycles_left -= 1;
                    continue;
                }
                if instr.is_memory_op() {
                    let result = core.cache.access(instr.address);
                    let hit = result == CacheAccessResult::Hit;
                    let stall = if hit {
                        0u64
                    } else {
                        self.memory.access_latency_cycles() as u64
                    };
                    self.metrics.record_access(CoreId(core_id), hit, stall);
                    instr.stage = PipelineStage::Memory;
                    if hit {
                        instr.stage_cycles_left = core.cache.hit_latency_cycles();
                    } else {
                        instr.stalled = true;
                        instr.stall_cycles_left = self.memory.access_latency_cycles();
                    }
                } else {
                    instr.stage = PipelineStage::Commit;
                    instr.stage_cycles_left = self.stage_cycles.commit_cycles;
                }
            }
        }

        // 4) Fetch stage: advance to Execute.
        for core_id in 0..self.num_cores {
            let core = &mut self.cores[core_id];
            for instr in core.pipeline.iter_mut() {
                if instr.stage != PipelineStage::Fetch {
                    continue;
                }
                if instr.stage_cycles_left > 0 {
                    instr.stage_cycles_left -= 1;
                    continue;
                }
                instr.stage = PipelineStage::Execute;
                instr.stage_cycles_left = self.stage_cycles.execute_cycles;
            }
        }

        // 5) Fetch new instructions from workload into pipeline (up to pipeline_width).
        for core_id in 0..self.num_cores {
            let core = &mut self.cores[core_id];
            while core.pipeline.len() < core.pipeline_width {
                let Some(mut instr) = core.workload.pop_front() else {
                    break;
                };
                instr.stage = PipelineStage::Fetch;
                instr.stage_cycles_left = self.stage_cycles.fetch_cycles;
                core.pipeline.push_back(instr);
            }
        }

        self.metrics.total_cycles = self.current_cycle;
    }

    /// Run until all cores have empty workload and empty pipeline.
    pub fn run_to_completion(&mut self) {
        loop {
            let busy = self.cores.iter().any(|c| !c.workload.is_empty() || !c.pipeline.is_empty());
            if !busy {
                break;
            }
            self.step();
        }
    }

    pub fn current_cycle(&self) -> Cycle {
        self.current_cycle
    }

    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    pub fn num_cores(&self) -> usize {
        self.num_cores
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workload::{build_workload, AccessPattern, WorkloadConfig};

    #[test]
    fn simulator_steps_and_drains_workload() {
        let cache_config = CacheConfig::default();
        let memory_config = MemoryConfig::default();
        let mut sim = Simulator::new(2, 2, cache_config, memory_config, 4);
        let workload = build_workload(
            2,
            WorkloadConfig {
                instructions_per_thread: 20,
                memory_fraction: 0.2,
                access_pattern: AccessPattern::Sequential,
                line_size: 64,
                cache_num_sets: 64,
                working_set_lines: 0,
            },
        );
        sim.load_workload(workload);
        sim.run_to_completion();
        assert!(sim.current_cycle() > 0);
        assert!(sim.metrics().total_cycles > 0);
    }

    #[test]
    fn simulator_tracks_memory_accesses() {
        let cache_config = CacheConfig::default();
        let memory_config = MemoryConfig::default();
        let mut sim = Simulator::new(1, 1, cache_config, memory_config, 4);
        let workload = build_workload(
            1,
            WorkloadConfig {
                instructions_per_thread: 100,
                memory_fraction: 0.5,
                access_pattern: AccessPattern::Sequential,
                line_size: 64,
                cache_num_sets: 64,
                working_set_lines: 0,
            },
        );
        sim.load_workload(workload);
        sim.run_to_completion();
        assert!(sim.metrics().total_memory_accesses > 0);
        assert!(sim.metrics().cache_hits + sim.metrics().cache_misses == sim.metrics().total_memory_accesses);
    }
}
