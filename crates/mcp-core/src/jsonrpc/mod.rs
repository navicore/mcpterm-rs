//! JSON-RPC utility functions for MCP

pub mod extractor;
pub mod splitter;

pub use extractor::extract_jsonrpc_objects;
pub use splitter::{split_jsonrpc_and_text, SplitContent};