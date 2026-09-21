//! The search worker: one long-lived thread that owns the index, the
//! matcher and every filesystem read, so the UI thread never waits on
//! a walk or a grep. The UI sends commands through a channel and the
//! worker answers with events pushed onto a shared queue that wakes
//! the event loop — the same arrangement the media caches and the
//! parse worker use.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::content;
use super::file_matcher::FileMatcher;
use super::index::{ScanFailure, WorkspaceIndex};
use super::types::{QueryGeneration, RootGeneration, SearchCommand, SearchEvent, SearchToken};

/// How the worker wakes the event loop: the same shape every
/// background worker in Oryx hands around.
pub type Waker = Arc<dyn Fn() + Send + Sync>;

/// How long a content query waits for the typist to settle, so one
/// grep answers a burst of keystrokes instead of each one. File
/// queries are pure memory and run at once.
const CONTENT_DEBOUNCE: Duration = Duration::from_millis(100);

/// The generations the UI stands on, read by the worker without
/// locking while it works. A channel message cannot reach a worker
/// busy grepping twenty thousand files, but these can, so a search the
/// UI has left behind stops between files rather than running on.
#[derive(Default)]
pub struct Cancel {
    root: AtomicU64,
    query: AtomicU64,
}

impl Cancel {
    /// Whether a result carrying `token` is still an answer to the
    /// standing question about the standing root.
    pub fn stands_on(&self, token: SearchToken) -> bool {
        self.root.load(Ordering::SeqCst) == token.root.0
            && self.query.load(Ordering::SeqCst) == token.query.0
    }

    /// Whether an index build under `generation` is still the one to
    /// finish.
    pub fn root_stands(&self, generation: RootGeneration) -> bool {
        self.root.load(Ordering::SeqCst) == generation.0
    }
}

/// The UI's handle on the worker: sends commands, keeps the token the
/// answers must carry, and drains the events the worker parked. The
/// thread lives as long as the handle's channel does.
pub struct WorkspaceSearch {
    tx: Sender<SearchCommand>,
    arrivals: Arc<Mutex<Vec<SearchEvent>>>,
    cancel: Arc<Cancel>,
    /// Generations issued so far; `token` is what an answer must carry.
    root: RootGeneration,
    query: QueryGeneration,
    token: SearchToken,
    /// The root last handed over, so a repeat `sync_root` is free.
    settled: Option<PathBuf>,
    handle: Option<JoinHandle<()>>,
}

impl WorkspaceSearch {
    /// Starts the worker behind a woken event loop.
    pub fn new(waker: Waker) -> WorkspaceSearch {
        let (tx, rx) = std::sync::mpsc::channel();
        let arrivals = Arc::new(Mutex::new(Vec::new()));
        let cancel = Arc::new(Cancel::default());
        let thread = std::thread::Builder::new()
            .name("workspace-search".to_string())
            .spawn({
                let arrivals = arrivals.clone();
                let cancel = cancel.clone();
                move || Worker::new(rx, arrivals, cancel, waker).run()
            })
            .expect("spawn workspace-search");
        WorkspaceSearch {
            tx,
            arrivals,
            cancel,
            root: RootGeneration(0),
            query: QueryGeneration(0),
            token: SearchToken::default(),
            settled: None,
            handle: Some(thread),
        }
    }

    /// The token an answer must carry right now; anything else in the
    /// event queue is stale and dropped by the caller.
    pub fn token(&self) -> SearchToken {
        self.token
    }

    /// Hands the worker a root, if it is not the one already standing.
    /// A change bumps the root generation, which abandons any walk or
    /// answer in flight for the old root.
    pub fn sync_root(&mut self, root: &Path) {
        if self.settled.as_deref() == Some(root) {
            return;
        }
        self.settled = Some(root.to_path_buf());
        self.bump_root();
        let _ = self.tx.send(SearchCommand::SetRoot {
            generation: self.root,
            root: root.to_path_buf(),
        });
    }

    /// Rebuilds the index of the root already standing, under a fresh
    /// generation so the old index's answers stop applying.
    pub fn refresh(&mut self) {
        let Some(root) = self.settled.clone() else {
            return;
        };
        self.bump_root();
        let _ = self.tx.send(SearchCommand::SetRoot {
            generation: self.root,
            root,
        });
    }

    fn bump_root(&mut self) {
        self.root = RootGeneration(self.root.0 + 1);
        self.cancel.root.store(self.root.0, Ordering::SeqCst);
        self.token.root = self.root;
    }

    /// Asks for the index's files ranked against a fuzzy query.
    pub fn search_files(&mut self, query: &str, limit: usize) {
        self.bump_query();
        let _ = self.tx.send(SearchCommand::SearchFiles {
            token: self.token,
            query: query.to_string(),
            limit,
        });
    }

    /// Asks for the index's files grepped for a plain or regex query.
    pub fn search_content(
        &mut self,
        query: &str,
        regex: bool,
        limit: usize,
        current_buffer: Option<super::types::BufferOverride>,
    ) {
        self.bump_query();
        let _ = self.tx.send(SearchCommand::SearchContent {
            token: self.token,
            query: query.to_string(),
            regex,
            limit,
            current_buffer,
        });
    }

    fn bump_query(&mut self) {
        self.query = QueryGeneration(self.query.0 + 1);
        self.cancel.query.store(self.query.0, Ordering::SeqCst);
        self.token.query = self.query;
    }

    /// Takes the events parked since the last call. The caller checks
    /// each answer's token against `token` before showing it; the
    /// worker cancels stale work, and this is the second wall.
    pub fn drain(&mut self) -> Vec<SearchEvent> {
        let mut queue = self.arrivals.lock().expect("search arrivals lock");
        std::mem::take(&mut *queue)
    }

    /// Stops the worker and waits for the thread to end. Safe to call
    /// twice; dropping the handle without this parks the thread on its
    /// channel until the process ends.
    pub fn shutdown(&mut self) {
        let _ = self.tx.send(SearchCommand::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for WorkspaceSearch {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// What the worker thread owns: the channel, the shared queues, the
/// index, the matcher and the standing root.
struct Worker {
    rx: Receiver<SearchCommand>,
    arrivals: Arc<Mutex<Vec<SearchEvent>>>,
    cancel: Arc<Cancel>,
    waker: Waker,
    index: Option<WorkspaceIndex>,
    root: Option<PathBuf>,
    files: FileMatcher,
}

impl Worker {
    fn new(
        rx: Receiver<SearchCommand>,
        arrivals: Arc<Mutex<Vec<SearchEvent>>>,
        cancel: Arc<Cancel>,
        waker: Waker,
    ) -> Worker {
        Worker {
            rx,
            arrivals,
            cancel,
            waker,
            index: None,
            root: None,
            files: FileMatcher::new(),
        }
    }

    /// Serves commands until shutdown. Commands that arrive while a
    /// content query waits out its debounce wait in the backlog and
    /// run in order once it fires; only a newer search ever displaces
    /// an older one waiting there.
    fn run(mut self) {
        let mut backlog: Vec<SearchCommand> = Vec::new();
        loop {
            let command = match backlog.first() {
                Some(_) => backlog.remove(0),
                None => match self.rx.recv() {
                    Ok(command) => command,
                    Err(_) => break,
                },
            };
            match command {
                SearchCommand::Shutdown => break,
                SearchCommand::SetRoot { generation, root } => self.set_root(generation, root),
                SearchCommand::Refresh { generation } => {
                    if let Some(root) = self.root.clone() {
                        self.set_root(generation, root);
                    }
                }
                SearchCommand::SearchFiles {
                    token,
                    query,
                    limit,
                } => self.search_files(token, &query, limit),
                SearchCommand::SearchContent {
                    token,
                    query,
                    regex,
                    limit,
                    current_buffer,
                } => {
                    let latest =
                        self.debounce(token, query, regex, limit, current_buffer, &mut backlog);
                    if let Some((token, query, regex, limit, current_buffer)) = latest {
                        self.search_content(token, &query, regex, limit, current_buffer);
                    }
                }
            }
        }
    }

    /// Waits out the content debounce, keeping only the newest query:
    /// each content query that arrives inside the window displaces the
    /// one waiting, while anything else lines up in the backlog. A
    /// query already superseded while waiting never runs at all.
    fn debounce(
        &self,
        token: SearchToken,
        query: String,
        regex: bool,
        limit: usize,
        current_buffer: Option<super::types::BufferOverride>,
        backlog: &mut Vec<SearchCommand>,
    ) -> Option<(
        SearchToken,
        String,
        bool,
        usize,
        Option<super::types::BufferOverride>,
    )> {
        let mut waiting = (token, query, regex, limit, current_buffer);
        let deadline = Instant::now() + CONTENT_DEBOUNCE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            match self.rx.recv_timeout(left) {
                Ok(SearchCommand::SearchContent {
                    token,
                    query,
                    regex,
                    limit,
                    current_buffer,
                }) => waiting = (token, query, regex, limit, current_buffer),
                Ok(SearchCommand::Shutdown) => backlog.push(SearchCommand::Shutdown),
                Ok(other) => backlog.push(other),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        self.cancel.stands_on(waiting.0).then_some(waiting)
    }

    fn set_root(&mut self, generation: RootGeneration, root: PathBuf) {
        self.emit(SearchEvent::IndexStarted { generation });
        let cancel = self.cancel.clone();
        match WorkspaceIndex::scan(&root, generation, &|| !cancel.root_stands(generation)) {
            Ok(index) => {
                // The generation on the built index and the standing
                // one must agree: either disagreeing means a newer
                // root took over while the walk ran.
                if index.generation() != generation || !self.cancel.root_stands(generation) {
                    return;
                }
                let files = index.len();
                self.root = Some(root);
                self.index = Some(index);
                self.emit(SearchEvent::IndexReady { generation, files });
            }
            Err(ScanFailure::Superseded) => {}
            Err(ScanFailure::Unreadable(message)) => {
                if !self.cancel.root_stands(generation) {
                    return;
                }
                self.index = None;
                self.emit(SearchEvent::Error {
                    token: None,
                    message,
                });
            }
        }
    }

    fn search_files(&mut self, token: SearchToken, query: &str, limit: usize) {
        if query.is_empty() || !self.cancel.stands_on(token) {
            self.emit(SearchEvent::FileResults {
                token,
                results: Vec::new(),
            });
            self.emit(SearchEvent::Finished {
                token,
                truncated: false,
                skipped: 0,
            });
            return;
        }
        let Some(index) = self.index.as_ref() else {
            self.emit(SearchEvent::FileResults {
                token,
                results: Vec::new(),
            });
            self.emit(SearchEvent::Finished {
                token,
                truncated: false,
                skipped: 0,
            });
            return;
        };
        if index.is_empty() {
            self.emit(SearchEvent::FileResults {
                token,
                results: Vec::new(),
            });
            self.emit(SearchEvent::Finished {
                token,
                truncated: false,
                skipped: 0,
            });
            return;
        }
        let cancel = self.cancel.clone();
        let (results, truncated) = self
            .files
            .rank(index, query, limit, &|| !cancel.stands_on(token));
        if !self.cancel.stands_on(token) {
            return;
        }
        self.emit(SearchEvent::FileResults { token, results });
        self.emit(SearchEvent::Finished {
            token,
            truncated,
            skipped: 0,
        });
    }

    fn search_content(
        &mut self,
        token: SearchToken,
        query: &str,
        regex: bool,
        limit: usize,
        current_buffer: Option<super::types::BufferOverride>,
    ) {
        if query.is_empty() || !self.cancel.stands_on(token) {
            self.emit(SearchEvent::ContentBatch {
                token,
                results: Vec::new(),
            });
            self.emit(SearchEvent::Finished {
                token,
                truncated: false,
                skipped: 0,
            });
            return;
        }
        let Some(index) = self.index.as_ref() else {
            self.emit(SearchEvent::ContentBatch {
                token,
                results: Vec::new(),
            });
            self.emit(SearchEvent::Finished {
                token,
                truncated: false,
                skipped: 0,
            });
            return;
        };
        // Batches stream straight onto the arrivals queue; the loop
        // wakes once a batch, the way the media caches wake once an
        // image.
        let arrivals = self.arrivals.clone();
        let waker = self.waker.clone();
        let cancel = self.cancel.clone();
        let outcome = content::search(
            index,
            query,
            regex,
            limit,
            current_buffer.as_ref(),
            &|| !cancel.stands_on(token),
            |batch| {
                arrivals
                    .lock()
                    .expect("search arrivals lock")
                    .push(SearchEvent::ContentBatch {
                        token,
                        results: batch,
                    });
                waker();
            },
        );
        if !self.cancel.stands_on(token) {
            return;
        }
        match outcome {
            Ok(outcome) => self.emit(SearchEvent::Finished {
                token,
                truncated: outcome.truncated,
                skipped: outcome.skipped,
            }),
            Err(message) => self.emit(SearchEvent::Error {
                token: Some(token),
                message,
            }),
        }
    }

    /// Parks an event for the UI and wakes the event loop.
    fn emit(&mut self, event: SearchEvent) {
        self.arrivals
            .lock()
            .expect("search arrivals lock")
            .push(event);
        (self.waker)();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A worker standing on a quiet waker, with a helper that waits
    /// for the event queue to hold something the probe accepts.
    struct Probe {
        search: WorkspaceSearch,
        /// Events that arrived but no assertion wanted yet; a later
        /// await still finds them, in arrival order.
        seen: Vec<SearchEvent>,
    }

    impl Probe {
        fn new() -> Probe {
            Probe {
                search: WorkspaceSearch::new(Arc::new(|| {})),
                seen: Vec::new(),
            }
        }

        /// Drains repeatedly until `want` accepts an event, or panics
        /// after a deadline the fastest machine never needs. Events
        /// nothing wants are kept for later awaits, since one drain can
        /// bring a whole answer at once.
        fn await_event(&mut self, want: impl Fn(&SearchEvent) -> bool) -> SearchEvent {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(at) = self.seen.iter().position(|e| want(e)) {
                    return self.seen.remove(at);
                }
                assert!(Instant::now() < deadline, "the worker never answered");
                self.seen.extend(self.search.drain());
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }

    fn fresh(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oryx-worker-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_root_indexes_and_reports_its_file_count() {
        let dir = fresh("ready");
        std::fs::write(dir.join("a.md"), "x").unwrap();
        std::fs::write(dir.join("b.rs"), "x").unwrap();
        let mut probe = Probe::new();
        probe.search.sync_root(&dir);
        match probe
            .await_event(|e| matches!(e, SearchEvent::IndexReady { files, .. } if *files == 2))
        {
            SearchEvent::IndexReady { generation, .. } => {
                assert_eq!(generation, probe.search.token().root);
            }
            _ => unreachable!("the probe only returns what it accepts"),
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_second_sync_of_the_same_root_is_free() {
        let dir = fresh("same-root");
        let mut probe = Probe::new();
        probe.search.sync_root(&dir);
        probe.await_event(|e| matches!(e, SearchEvent::IndexReady { .. }));
        let before = probe.search.token();
        probe.search.sync_root(&dir);
        assert_eq!(probe.search.token(), before, "no generation moved");
        std::thread::sleep(Duration::from_millis(50));
        assert!(probe.search.drain().is_empty(), "no walk ran again");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Root A indexes, then root B: B's generation overtakes, and no
    /// event after B's readiness claims A's generation — a slow A walk
    /// must never land as the current index.
    #[test]
    fn a_newer_root_generation_buries_an_older_ready() {
        let a = fresh("older");
        let b = fresh("newer");
        std::fs::write(a.join("old.md"), "x").unwrap();
        std::fs::write(b.join("new.md"), "x").unwrap();
        let mut probe = Probe::new();
        probe.search.sync_root(&a);
        probe.await_event(|e| matches!(e, SearchEvent::IndexReady { .. }));
        probe.search.sync_root(&b);
        let buried = probe.search.token().root.0 - 1;
        match probe.await_event(|e| matches!(e, SearchEvent::IndexReady { .. })) {
            SearchEvent::IndexReady { generation, files } => {
                assert_eq!(generation, probe.search.token().root);
                assert_eq!(files, 1);
            }
            _ => unreachable!("the probe only returns what it accepts"),
        }
        // A rescan of A issued now would run under a new generation;
        // nothing arriving after B may carry A's old one. The events
        // buffered before B's readiness are A's by right, so they are
        // dropped before the watch begins.
        std::thread::sleep(Duration::from_millis(100));
        probe.seen.clear();
        let mut rest = std::mem::take(&mut probe.seen);
        rest.extend(probe.search.drain());
        for event in rest {
            let claims = match &event {
                SearchEvent::IndexStarted { generation }
                | SearchEvent::IndexReady { generation, .. } => Some(*generation),
                SearchEvent::FileResults { token, .. }
                | SearchEvent::ContentBatch { token, .. }
                | SearchEvent::Finished { token, .. } => Some(token.root),
                SearchEvent::Error { .. } => None,
            };
            if let Some(generation) = claims {
                assert_ne!(
                    generation.0, buried,
                    "an event from the abandoned root landed late"
                );
            }
        }
        std::fs::remove_dir_all(&a).unwrap();
        std::fs::remove_dir_all(&b).unwrap();
    }

    #[test]
    fn an_unreadable_root_reports_an_error_without_panicking() {
        let missing = fresh("missing");
        std::fs::remove_dir_all(&missing).unwrap();
        let mut probe = Probe::new();
        probe.search.sync_root(&missing);
        match probe.await_event(|e| matches!(e, SearchEvent::Error { .. })) {
            SearchEvent::Error { token, .. } => assert!(token.is_none()),
            _ => unreachable!("the probe only returns what it accepts"),
        }
    }

    #[test]
    fn a_refresh_rewalks_under_a_fresh_generation() {
        let dir = fresh("refresh");
        std::fs::write(dir.join("one.md"), "x").unwrap();
        let mut probe = Probe::new();
        probe.search.sync_root(&dir);
        probe.await_event(|e| matches!(e, SearchEvent::IndexReady { files, .. } if *files == 1));
        std::fs::write(dir.join("two.md"), "x").unwrap();
        probe.search.refresh();
        match probe
            .await_event(|e| matches!(e, SearchEvent::IndexReady { files, .. } if *files == 2))
        {
            SearchEvent::IndexReady { generation, .. } => {
                assert_eq!(generation, probe.search.token().root);
            }
            _ => unreachable!("the probe only returns what it accepts"),
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_empty_file_query_answers_empty_and_finishes() {
        let dir = fresh("empty-query");
        std::fs::write(dir.join("a.md"), "x").unwrap();
        let mut probe = Probe::new();
        probe.search.sync_root(&dir);
        probe.await_event(|e| matches!(e, SearchEvent::IndexReady { .. }));
        probe.search.search_files("", 100);
        match probe.await_event(|e| matches!(e, SearchEvent::FileResults { .. })) {
            SearchEvent::FileResults { token, results } => {
                assert_eq!(token, probe.search.token());
                assert!(results.is_empty());
            }
            _ => unreachable!("the probe only returns what it accepts"),
        }
        match probe.await_event(|e| matches!(e, SearchEvent::Finished { .. })) {
            SearchEvent::Finished {
                token,
                truncated,
                skipped,
            } => {
                assert_eq!(token, probe.search.token());
                assert!(!truncated);
                assert_eq!(skipped, 0);
            }
            _ => unreachable!("the probe only returns what it accepts"),
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_query_answers_through_the_worker_ranked() {
        let dir = fresh("file-query");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        for f in ["main.rs", "src/main.rs", "notes.md"] {
            std::fs::write(dir.join(f), "x").unwrap();
        }
        let mut probe = Probe::new();
        probe.search.sync_root(&dir);
        probe.await_event(|e| matches!(e, SearchEvent::IndexReady { .. }));
        probe.search.search_files("main", 100);
        match probe.await_event(|e| matches!(e, SearchEvent::FileResults { .. })) {
            SearchEvent::FileResults { token, results } => {
                assert_eq!(token, probe.search.token());
                let paths: Vec<String> = results
                    .iter()
                    .map(|hit| hit.relative_path.to_string())
                    .collect();
                assert_eq!(paths, ["main.rs", "src/main.rs"]);
            }
            _ => unreachable!("the probe only returns what it accepts"),
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_content_query_answers_through_the_worker_in_batches() {
        let dir = fresh("content-query");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/a.rs"), "one\nUserService::new\ntwo\n").unwrap();
        std::fs::write(dir.join("src/b.rs"), "UserService again\n").unwrap();
        let mut probe = Probe::new();
        probe.search.sync_root(&dir);
        probe.await_event(|e| matches!(e, SearchEvent::IndexReady { .. }));
        probe.search.search_content("UserService", false, 500, None);
        let mut lines = Vec::new();
        loop {
            match probe.await_event(|e| matches!(e, SearchEvent::ContentBatch { .. })) {
                SearchEvent::ContentBatch { token, results } => {
                    assert_eq!(token, probe.search.token());
                    for hit in results {
                        lines.push((hit.relative_path.to_string(), hit.line_number));
                    }
                }
                _ => unreachable!("the probe only returns what it accepts"),
            }
            // Stop once the run finished; the queue drains in order.
            if probe
                .seen
                .iter()
                .any(|e| matches!(e, SearchEvent::Finished { .. }))
            {
                break;
            }
        }
        assert_eq!(
            lines,
            [("src/a.rs".to_string(), 2), ("src/b.rs".to_string(), 1),]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_invalid_regex_reports_through_the_worker() {
        let dir = fresh("invalid-regex");
        std::fs::write(dir.join("a.txt"), "text\n").unwrap();
        let mut probe = Probe::new();
        probe.search.sync_root(&dir);
        probe.await_event(|e| matches!(e, SearchEvent::IndexReady { .. }));
        probe.search.search_content("foo(", true, 500, None);
        match probe.await_event(|e| matches!(e, SearchEvent::Error { .. })) {
            SearchEvent::Error { token, message } => {
                assert!(token.is_some(), "the pattern's own error, not the root's");
                assert!(!message.is_empty());
            }
            _ => unreachable!("the probe only returns what it accepts"),
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A content query fired and immediately superseded: whatever the
    /// abandoned run parked before the newer query stood is gone with
    /// the newer query's answer, and nothing the worker said under the
    /// old token lands after the new one stood — the UI's token check
    /// drops exactly these.
    #[test]
    fn a_superseded_content_query_never_lands_after_its_replacement() {
        let dir = fresh("overtake");
        std::fs::create_dir_all(dir.join("many")).unwrap();
        for n in 0..300 {
            std::fs::write(dir.join(format!("many/f{n:03}.txt")), "hit\n").unwrap();
        }
        std::fs::write(dir.join("needle.txt"), "the needle rests here\n").unwrap();
        let mut probe = Probe::new();
        probe.search.sync_root(&dir);
        probe.await_event(|e| matches!(e, SearchEvent::IndexReady { .. }));
        probe.search.search_content("hit", false, 500, None);
        let overtaken = probe.search.token();
        let before = probe.seen.len();
        probe.search.search_content("needle", false, 500, None);
        let current = probe.search.token();
        assert_ne!(overtaken, current);
        match probe
            .await_event(|e| matches!(e, SearchEvent::Finished { token, .. } if *token == current))
        {
            SearchEvent::Finished { token, .. } => assert_eq!(token, current),
            _ => unreachable!("the probe only returns what it accepts"),
        }
        std::thread::sleep(Duration::from_millis(150));
        probe.seen.extend(probe.search.drain());
        for event in &probe.seen[before..] {
            let carried = match event {
                SearchEvent::FileResults { token, .. }
                | SearchEvent::ContentBatch { token, .. }
                | SearchEvent::Finished { token, .. } => Some(*token),
                _ => None,
            };
            assert_ne!(
                carried,
                Some(overtaken),
                "the abandoned query's answer landed after its replacement stood"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn shutdown_stops_the_thread() {
        let mut search = WorkspaceSearch::new(Arc::new(|| {}));
        search.shutdown();
        assert!(search.handle.is_none(), "the thread was joined");
        // A second shutdown is quiet.
        search.shutdown();
    }
}
