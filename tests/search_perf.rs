//! Workspace search performance against its budgets. Ignored by
//! default; run in release mode locally with:
//!   cargo test --release --test search_perf -- --ignored --nocapture --test-threads=1
//! Parallel test threads contend for cores and skew every timing, and
//! a debug build answers for the allocator more than for the search.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use oryx::workspace_search::{SearchEvent, WorkspaceSearch, MAX_CONTENT_HITS, MAX_FILE_RESULTS};

/// Tree sizes: a small project, a real one, a monorepo.
const TIERS: &[usize] = &[1_000, 10_000, 100_000];

/// A tree of `files` files over folders of a hundred, each file
/// carrying one common phrase so a content query has real work.
fn tree(files: usize) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oryx-search-perf-{}-{files}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let made = Instant::now();
    for n in 0..files {
        let folder = dir.join(format!("d{:03}", n / 100));
        if n % 100 == 0 {
            std::fs::create_dir_all(&folder).unwrap();
        }
        let name = format!("file{n:06}.txt");
        std::fs::write(
            folder.join(&name),
            format!("alpha {name} bravo the needle rests charlie\n"),
        )
        .unwrap();
    }
    println!("generated {files} files in {:?}", made.elapsed());
    dir
}

/// Waits for an event the probe accepts, parking everything that
/// arrives in between.
fn await_event(
    search: &mut WorkspaceSearch,
    seen: &mut Vec<SearchEvent>,
    want: impl Fn(&SearchEvent) -> bool + Copy,
) {
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        seen.extend(search.drain());
        if seen.iter().any(want) {
            return;
        }
        assert!(Instant::now() < deadline, "the worker never answered");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
#[ignore]
fn the_index_the_queries_and_the_cancel_hold_their_budgets() {
    for files in TIERS {
        let dir = tree(*files);
        let mut search = WorkspaceSearch::new(Arc::new(|| {}));
        let mut seen = Vec::new();

        let started = Instant::now();
        search.sync_root(&dir);
        await_event(&mut search, &mut seen, |e| {
            matches!(e, SearchEvent::IndexReady { .. })
        });
        let indexed = started.elapsed();
        println!("{files} files: indexed in {indexed:?}");

        // A warm fuzzy query over the whole index.
        let started = Instant::now();
        search.search_files("file0423", MAX_FILE_RESULTS);
        await_event(&mut search, &mut seen, |e| {
            matches!(e, SearchEvent::FileResults { .. })
        });
        let file_query = started.elapsed();
        println!("{files} files: warm file query in {file_query:?}");

        // A content query: the first batch and the whole answer.
        let started = Instant::now();
        search.search_content("the needle rests", false, MAX_CONTENT_HITS, None);
        await_event(&mut search, &mut seen, |e| {
            matches!(e, SearchEvent::ContentBatch { .. })
        });
        let first_batch = started.elapsed();
        await_event(&mut search, &mut seen, |e| {
            matches!(e, SearchEvent::Finished { .. })
        });
        let content_total = started.elapsed();
        println!(
            "{files} files: first content batch in {first_batch:?}, whole grep in {content_total:?}"
        );

        // Cancellation: a fresh query issued over a running one, and
        // nothing the abandoned run still says lands after the fresh
        // one stood.
        let started = Instant::now();
        search.search_content("the needle rests", false, MAX_CONTENT_HITS, None);
        let overtaken = search.token();
        search.search_content("charlie", false, MAX_CONTENT_HITS, None);
        let current = search.token();
        assert_ne!(overtaken, current);
        seen.clear();
        await_event(&mut search, &mut seen, |e| {
            matches!(e, SearchEvent::Finished { .. })
        });
        let cancel = started.elapsed();
        for event in search.drain() {
            let carried = match event {
                SearchEvent::FileResults { token, .. }
                | SearchEvent::ContentBatch { token, .. }
                | SearchEvent::Finished { token, .. } => Some(token),
                _ => None,
            };
            assert_ne!(
                carried,
                Some(overtaken),
                "the abandoned run's answer landed after its replacement"
            );
        }
        println!("{files} files: supersede answered in {cancel:?}");

        // The budgets: generous walls, crossed only by a regression.
        if *files <= 10_000 {
            assert!(
                indexed < Duration::from_secs(20),
                "the walk took {indexed:?}"
            );
        } else {
            assert!(
                indexed < Duration::from_secs(120),
                "the walk took {indexed:?}"
            );
        }
        assert!(
            file_query < Duration::from_secs(2),
            "the query took {file_query:?}"
        );
        assert!(
            first_batch < Duration::from_secs(10),
            "the first batch took {first_batch:?}"
        );

        search.shutdown();
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
