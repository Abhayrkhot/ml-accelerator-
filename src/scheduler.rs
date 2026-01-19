//! Thread scheduling model: round-robin assignment of threads to cores.

use crate::core::{CoreId, ThreadId};

/// Maps threads to cores and decides which thread runs on which core each cycle.
/// Simplified: round-robin assignment (thread T runs on core T % N).
pub struct Scheduler {
    num_cores: usize,
    num_threads: usize,
}

impl Scheduler {
    pub fn new(num_cores: usize, num_threads: usize) -> Self {
        Self {
            num_cores,
            num_threads,
        }
    }

    /// Returns the core that should run the given thread (round-robin).
    pub fn thread_to_core(&self, thread_id: ThreadId) -> CoreId {
        CoreId(thread_id.0 % self.num_cores)
    }

    /// Returns the thread assigned to run on the given core for the current scheduling quantum.
    /// We use a simple model: core K runs thread K, K+N, K+2N, ... (round-robin by core).
    pub fn core_to_thread(&self, core_id: CoreId) -> Option<ThreadId> {
        if core_id.0 < self.num_cores {
            Some(ThreadId(core_id.0 % self.num_threads.max(1)))
        } else {
            None
        }
    }

    pub fn num_cores(&self) -> usize {
        self.num_cores
    }

    pub fn num_threads(&self) -> usize {
        self.num_threads
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_round_robin_two_cores_two_threads() {
        let s = Scheduler::new(2, 2);
        assert_eq!(s.thread_to_core(ThreadId(0)), CoreId(0));
        assert_eq!(s.thread_to_core(ThreadId(1)), CoreId(1));
    }

    #[test]
    fn scheduler_round_robin_four_threads_two_cores() {
        let s = Scheduler::new(2, 4);
        assert_eq!(s.thread_to_core(ThreadId(0)), CoreId(0));
        assert_eq!(s.thread_to_core(ThreadId(1)), CoreId(1));
        assert_eq!(s.thread_to_core(ThreadId(2)), CoreId(0));
        assert_eq!(s.thread_to_core(ThreadId(3)), CoreId(1));
    }

    #[test]
    fn scheduler_core_to_thread() {
        let s = Scheduler::new(2, 2);
        assert_eq!(s.core_to_thread(CoreId(0)), Some(ThreadId(0)));
        assert_eq!(s.core_to_thread(CoreId(1)), Some(ThreadId(1)));
    }

    #[test]
    fn scheduler_single_core() {
        let s = Scheduler::new(1, 4);
        assert_eq!(s.thread_to_core(ThreadId(0)), CoreId(0));
        assert_eq!(s.thread_to_core(ThreadId(3)), CoreId(0));
    }
}
