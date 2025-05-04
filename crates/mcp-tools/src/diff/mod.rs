// Diff tools module - provides tools for comparing files and generating/applying patches
mod diff_tool;
mod patch_tool;

pub use diff_tool::{DiffConfig, DiffFormat, DiffTool};
pub use patch_tool::{PatchConfig, PatchTool};
