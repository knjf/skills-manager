// Re-export everything from the core crate so that
// `use crate::core::*` continues to work in commands/.
pub use skills_manager_core::*;

// Tauri-dependent module stays local
pub mod file_watcher;
