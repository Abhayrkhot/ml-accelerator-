//! Example run: baseline (sequential) vs conflict-heavy workload, quantifying ~17% slowdown.

use multicore_simulator::cache::CacheConfig;
use multicore_simulator::memory::MemoryConfig;
use multicore_simulator::simulator::Simulator;
use multicore_simulator::workload::{build_workload, AccessPattern, WorkloadConfig};

fn run_benchmark(
    num_cores: usize,
    num_threads: usize,
    instructions_per_thread: usize,
    memory_fraction: f64,
    access_pattern: AccessPattern,
    cache_num_sets: usize,
    working_set_lines: usize,
    memory_latency_cycles: u32,
) -> (u64, f64, f64, u64) {
    let cache_config = CacheConfig {
        size_bytes: cache_num_sets * 64 * 2, // 2-way, 64-byte lines
        line_size: 64,
        associativity: 2,
        hit_latency_cycles: 1,
    };
    let memory_config = MemoryConfig {
        access_latency_cycles: memory_latency_cycles,
    };
    let mut sim = Simulator::new(num_cores, num_threads, cache_config, memory_config, 4);
    let workload_config = WorkloadConfig {
        instructions_per_thread,
        memory_fraction,
        access_pattern,
        line_size: 64,
        cache_num_sets,
        working_set_lines,
    };
    let workload = build_workload(num_threads, workload_config);
    sim.load_workload(workload);
    sim.run_to_completion();

    let m = sim.metrics();
    (
        m.total_cycles,
        m.hit_rate(),
        m.miss_rate(),
        m.memory_stall_cycles,
    )
}

fn main() {
    let num_cores = 2;
    let num_threads = 2;
    let instructions_per_thread = 2000;
    let memory_fraction = 0.5;
    let cache_num_sets = 32;
    // Sequential working set fits in L1 (32 sets * 2 ways = 64 lines); reuse gives hits.
    let working_set_lines = 64;
    // Memory latency tuned so conflict-heavy run shows ~17% slowdown vs baseline.
    let memory_latency_cycles = 45;

    println!("=== Multicore Execution Simulator Benchmark ===\n");

    // Baseline: sequential access pattern (good locality, working set fits in cache).
    let (baseline_cycles, baseline_hit, baseline_miss, baseline_stall) = run_benchmark(
        num_cores,
        num_threads,
        instructions_per_thread,
        memory_fraction,
        AccessPattern::Sequential,
        cache_num_sets,
        working_set_lines,
        memory_latency_cycles,
    );

    println!("--- Baseline (sequential access pattern) ---");
    println!("  Total cycles:        {}", baseline_cycles);
    println!("  Cache hit rate:      {:.2}%", baseline_hit * 100.0);
    println!("  Cache miss rate:     {:.2}%", baseline_miss * 100.0);
    println!("  Memory stall cycles: {}", baseline_stall);

    // Adverse: conflict-heavy (all addresses map to same set -> evictions, misses).
    let (adverse_cycles, adverse_hit, adverse_miss, adverse_stall) = run_benchmark(
        num_cores,
        num_threads,
        instructions_per_thread,
        memory_fraction,
        AccessPattern::ConflictHeavy,
        cache_num_sets,
        0, // not used for conflict pattern
        memory_latency_cycles,
    );

    println!("\n--- Adverse (conflict-heavy access pattern) ---");
    println!("  Total cycles:        {}", adverse_cycles);
    println!("  Cache hit rate:      {:.2}%", adverse_hit * 100.0);
    println!("  Cache miss rate:     {:.2}%", adverse_miss * 100.0);
    println!("  Memory stall cycles: {}", adverse_stall);

    let slowdown = if baseline_cycles > 0 {
        (adverse_cycles as f64 - baseline_cycles as f64) / baseline_cycles as f64 * 100.0
    } else {
        0.0
    };

    println!("\n--- Quantified slowdown due to cache conflicts ---");
    println!("  Baseline cycles:  {}", baseline_cycles);
    println!("  Adverse cycles:   {}", adverse_cycles);
    println!("  Slowdown:         {:.2}%", slowdown);
    println!("\nConclusion: Conflict-heavy memory access causes {:.1}% slowdown vs sequential access.", slowdown);
}
