//! Cœur du noyau. Ordre de construction : boot → mem → (timer, sched, threads) → (caps, ipc) → syscalls.
pub mod boot;
pub mod caps;
pub mod ipc;
pub mod mem;
pub mod paging;
pub mod sched;
pub mod syscalls;
pub mod threads;
pub mod timer;
