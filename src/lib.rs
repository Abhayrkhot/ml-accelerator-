//! Multicore execution simulator: thread scheduling, cache contention, memory latency.

pub mod cache;
pub mod core;
pub mod memory;
pub mod metrics;
pub mod scheduler;
pub mod simulator;
pub mod workload;
