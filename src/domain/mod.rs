//! Pure domain data types, sliced by feature. No I/O, no UI, no business logic.
//!
//! Each feature owns a folder here mirroring `system/`, `app/`, and `ui/` (AGENTS.md §2).

pub mod profile;
pub mod tasks;
