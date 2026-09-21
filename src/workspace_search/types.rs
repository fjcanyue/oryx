//! The vocabulary between the UI and the search worker: the commands
//! the UI sends, the events that come back, and the generation tokens
//! that keep a late answer from an abandoned question out of the
//! display. Nothing here may know about the UI, so the worker stays a
//! plain library and can be replaced whole.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

/// Identifies one index build. The root changing, or an explicit
/// refresh, starts a build under a new generation, and any result from
/// an older one is stale the moment the newer generation is issued.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct RootGeneration(pub u64);

/// Identifies one query. Every change of the search text, mode or
/// buffer override starts a query under a new generation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct QueryGeneration(pub u64);

/// The pairing a result must still carry to be shown: both the root it
/// was computed against and the question it answers. The UI bumps the
/// halves it invalidates and discards anything that no longer matches.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SearchToken {
    pub root: RootGeneration,
    pub query: QueryGeneration,
}

/// One ranked answer of a fuzzy file query.
pub struct FileHit {
    /// The file's path below the workspace root, `/`-separated.
    pub relative_path: Arc<str>,
    /// Where in `relative_path` the file's own name starts, so the
    /// directory above it can be drawn apart from it.
    pub basename_start: u32,
    /// The matcher's score; higher ranks first.
    pub score: u32,
    /// Byte offsets into `relative_path` of the characters the query
    /// matched, for highlighting.
    pub matched: Vec<u32>,
}

/// One matching line of a content query. A line holding several
/// matches is one hit whose `ranges` list them all.
pub struct ContentHit {
    /// The file's path below the workspace root, `/`-separated.
    pub relative_path: Arc<str>,
    /// The line's number, one-based as editors count them.
    pub line_number: u64,
    /// The line as the searcher decoded it, without its terminator.
    pub line_text: Arc<str>,
    /// Byte ranges within `line_text` that matched.
    pub ranges: Vec<Range<usize>>,
}

/// The current document's unsaved text, so a content search answers
/// against what the reader sees rather than what the disk holds. Oryx
/// edits one document at a time, so one override covers it.
pub struct BufferOverride {
    /// The document's path below the workspace root, `/`-separated;
    /// `None` when the document has not been saved anywhere yet.
    pub relative_path: Option<Arc<str>>,
    pub text: Arc<str>,
}

/// What the UI asks the worker to do.
pub enum SearchCommand {
    /// Walk this root and make it the index. A newer generation
    /// abandons a build already running.
    SetRoot {
        generation: RootGeneration,
        root: PathBuf,
    },
    /// Rank the index's files against a fuzzy query.
    SearchFiles {
        token: SearchToken,
        query: String,
        limit: usize,
    },
    /// Grep the index's files for a plain or regular-expression query.
    SearchContent {
        token: SearchToken,
        query: String,
        regex: bool,
        limit: usize,
        current_buffer: Option<BufferOverride>,
    },
    /// Rebuild the index of the root already standing.
    Refresh { generation: RootGeneration },
    /// Stop the worker; the thread ends.
    Shutdown,
}

/// What the worker tells the UI. Events carrying a token are answers
/// and must be checked against the token the UI stands on; the rest
/// report on the index itself.
pub enum SearchEvent {
    /// A walk of the root has begun.
    IndexStarted { generation: RootGeneration },
    /// The walk finished; the index holds `files` paths.
    IndexReady {
        generation: RootGeneration,
        files: usize,
    },
    /// The whole answer of a file query, ranked.
    FileResults {
        token: SearchToken,
        results: Vec<FileHit>,
    },
    /// One batch of a content query's answer, in file order; more
    /// batches and a `Finished` follow.
    ContentBatch {
        token: SearchToken,
        results: Vec<ContentHit>,
    },
    /// The query's answer is complete.
    Finished {
        token: SearchToken,
        truncated: bool,
        /// Files that could not be read, skipped rather than failed.
        skipped: usize,
    },
    /// The root could not be walked at all, or the pattern is invalid.
    Error {
        token: Option<SearchToken>,
        message: String,
    },
}
