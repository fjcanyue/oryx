//! Workspace search: a fuzzy file finder and a content grep over the
//! sidebar's Files root, in the spirit of fd with fzf and of ripgrep,
//! as a library — no external tool is run, no index outlives the
//! walk, nothing reads a file before it must. The UI sends commands
//! and receives events; this module owns the walking, the matching
//! and the reading, all off the UI thread.

mod file_matcher;
mod index;
mod types;
mod worker;

pub use types::{
    BufferOverride, ContentHit, FileHit, QueryGeneration, RootGeneration, SearchCommand,
    SearchEvent, SearchToken,
};
pub use worker::WorkspaceSearch;
