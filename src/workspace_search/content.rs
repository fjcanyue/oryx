//! The content grep: ripgrep's own search stack — `grep-regex` for the
//! pattern, `grep-searcher` for the reading — run file by file over
//! the index. Plain queries are fixed strings, smart case applies to
//! both modes, a NUL byte ends a file's search, and no memory map is
//! taken: buffered reads only. Hits stream out in batches so the
//! results list fills as the walk goes; one line several matches
//! answers once, with every range.

use std::io;
use std::ops::Range;
use std::sync::Arc;

use grep_matcher::Matcher as _;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::Lossy;
use grep_searcher::{BinaryDetection, MmapChoice, SearcherBuilder};

use super::index::WorkspaceIndex;
use super::types::{BufferOverride, ContentHit};

/// How many hits ride one batch to the UI; the worker parks a batch
/// and wakes the loop, rather than one event a match.
pub const BATCH: usize = 30;

/// How a content query ended.
pub struct ContentOutcome {
    /// The limit cut the answer short.
    pub truncated: bool,
    /// Files that could not be read, skipped rather than failed.
    pub skipped: usize,
}

/// Greps the index's files for `query`. Each matching line becomes one
/// `ContentHit` in file order; `emit` receives them in batches.
/// The dirty buffer stands in for its file's disk copy. `superseded`
/// is polled between files so a query the UI left behind stops without
/// waiting for the channel it cannot see. An invalid pattern answers
/// `Err` with the message; nothing is searched then.
pub fn search(
    index: &WorkspaceIndex,
    query: &str,
    regex: bool,
    limit: usize,
    buffer: Option<&BufferOverride>,
    superseded: &dyn Fn() -> bool,
    mut emit: impl FnMut(Vec<ContentHit>),
) -> Result<ContentOutcome, String> {
    let matcher = RegexMatcherBuilder::new()
        .fixed_strings(!regex)
        .case_smart(true)
        .build(query)
        .map_err(|e| e.to_string())?;
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .binary_detection(BinaryDetection::quit(0))
        .multi_line(false)
        .memory_map(MmapChoice::never())
        .build();
    let mut hits: Vec<ContentHit> = Vec::new();
    // Hits counted toward the limit whether or not their batch has
    // ridden out: the batch flush empties `hits`, the limit stands.
    let mut taken = 0usize;
    let mut truncated = false;
    let mut skipped = 0;
    'files: for file in index.files() {
        if superseded() {
            // The caller drops everything from a superseded run; no
            // batch worth emitting remains.
            return Ok(ContentOutcome {
                truncated: false,
                skipped,
            });
        }
        let absolute = index.absolute(file);
        // A matching buffer override answers for the file's disk copy.
        let from_buffer = buffer.and_then(|b| {
            (b.relative_path.as_deref() == Some(&*file.relative_path)).then(|| b.text.clone())
        });
        {
            let mut take_line = |line_number: u64, line: &str| -> Result<bool, io::Error> {
                let trimmed = line.trim_end_matches(['\r', '\n']);
                // Where in the line the pattern landed; the searcher
                // matched the whole line, the ranges narrow it.
                let mut ranges: Vec<Range<usize>> = Vec::new();
                let mut any = false;
                let _ = matcher.find_iter(trimmed.as_bytes(), |m| {
                    any = true;
                    if !m.is_empty() && m.end() <= trimmed.len() {
                        ranges.push(m.start()..m.end());
                    }
                    true
                });
                if !any {
                    return Ok(true);
                }
                if taken >= limit {
                    truncated = true;
                    return Ok(false);
                }
                hits.push(ContentHit {
                    relative_path: Arc::from(&*file.relative_path),
                    line_number,
                    line_text: Arc::from(trimmed),
                    ranges,
                });
                taken += 1;
                // A full batch rides out at once, mid-file included,
                // so one big file streams as it goes.
                if hits.len() >= BATCH {
                    emit(std::mem::take(&mut hits));
                }
                Ok(true)
            };
            let sink = Lossy(&mut take_line);
            let searched = match from_buffer {
                Some(text) => searcher.search_slice(&matcher, text.as_bytes(), sink),
                None => searcher.search_path(&matcher, &absolute, sink),
            };
            if searched.is_err() {
                skipped += 1;
            }
        }
        if truncated {
            break 'files;
        }
    }
    if !hits.is_empty() {
        emit(hits);
    }
    Ok(ContentOutcome { truncated, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_search::index::WorkspaceIndex;
    use crate::workspace_search::types::RootGeneration;

    const NEVER: fn() -> bool = || false;

    fn fresh(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("oryx-grep-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Indexes the tree and answers one query's hits as
    /// `(path, line, text)` triples.
    fn grep(
        dir: &std::path::Path,
        files: &[(&str, &str)],
        query: &str,
        regex: bool,
    ) -> Vec<(String, u64, String)> {
        for (name, text) in files {
            if let Some(parent) = std::path::Path::new(name).parent() {
                std::fs::create_dir_all(dir.join(parent)).unwrap();
            }
            std::fs::write(dir.join(name), text).unwrap();
        }
        let index = WorkspaceIndex::scan(dir, RootGeneration(0), &NEVER).unwrap();
        let mut all: Vec<(String, u64, String)> = Vec::new();
        let outcome = search(&index, query, regex, 500, None, &NEVER, |batch| {
            for hit in batch {
                all.push((
                    hit.relative_path.to_string(),
                    hit.line_number,
                    hit.line_text.to_string(),
                ));
            }
        })
        .unwrap();
        assert!(!outcome.truncated);
        assert_eq!(outcome.skipped, 0);
        all
    }

    #[test]
    fn a_plain_query_finds_lines_with_numbers_and_text() {
        let dir = fresh("plain");
        let found = grep(
            &dir,
            &[
                ("a.rs", "one\nUserService::new\ntwo\n"),
                ("b.md", "see UserService here\n"),
            ],
            "UserService",
            false,
        );
        assert_eq!(
            found,
            [
                ("a.rs".to_string(), 2, "UserService::new".to_string()),
                ("b.md".to_string(), 1, "see UserService here".to_string()),
            ]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn plain_dots_and_stars_are_literal() {
        let dir = fresh("literal");
        let found = grep(&dir, &[("t.txt", "a.b\naxb\na*b\n")], "a.b", false);
        assert_eq!(found.len(), 1, "only the literal dot line");
        assert_eq!(found[0].2, "a.b");
        let found = grep(&dir, &[("t.txt", "x*y\nabc\n")], "x*y", false);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].2, "x*y");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn regex_dots_and_stars_do_their_work() {
        let dir = fresh("regex");
        let found = grep(&dir, &[("t.txt", "axb\nab\na-b\n")], "a.b", true);
        assert_eq!(found.len(), 2, "a dot wants a character of its own");
        let found = grep(&dir, &[("t.txt", "ab\naaab\nb\n")], "a*b", true);
        assert_eq!(
            found.iter().map(|f| f.2.clone()).collect::<Vec<_>>(),
            ["ab", "aaab", "b"],
            "a star can fold to nothing, so a lone b matches too"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn smart_case_holds_for_both_modes() {
        let dir = fresh("case");
        // One word a line, so the counts are line counts.
        let files = &[("t.txt", "panel\nPANEL\nPanel\n")];
        assert_eq!(grep(&dir, files, "panel", false).len(), 3);
        assert_eq!(grep(&dir, files, "Panel", false).len(), 1);
        assert_eq!(grep(&dir, files, "panel", true).len(), 3);
        assert_eq!(grep(&dir, files, "Panel", true).len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unicode_text_matches_across_bytes() {
        let dir = fresh("unicode");
        let found = grep(
            &dir,
            &[("t.md", "hello 世界\nnope\n再 世界 once\n")],
            "世界",
            false,
        );
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].1, 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_invalid_regex_answers_an_error_and_searches_nothing() {
        let dir = fresh("invalid");
        std::fs::write(dir.join("t.txt"), "anything").unwrap();
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        let mut seen = 0;
        let outcome = search(&index, "foo(", true, 10, None, &NEVER, |_| seen += 1);
        assert!(outcome.is_err());
        assert_eq!(seen, 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_binary_file_is_skipped_quietly() {
        let dir = fresh("binary");
        let found = grep(
            &dir,
            &[
                ("a.txt", "before\n"),
                ("blob.bin", "text\0with nul\nmatches nothing visible\n"),
                ("b.txt", "after\n"),
            ],
            "before",
            false,
        );
        assert_eq!(found.len(), 1);
        // And the needle past the NUL never surfaces.
        let found = grep(
            &dir,
            &[("blob.bin", "aaa\0bbb UserService ccc\n")],
            "UserService",
            false,
        );
        assert!(found.is_empty(), "the search quit at the NUL byte");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn one_line_many_matches_is_one_hit_with_ranges() {
        let dir = fresh("ranges");
        std::fs::write(dir.join("t.txt"), "user user user\n").unwrap();
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        let mut hits: Vec<ContentHit> = Vec::new();
        search(&index, "user", false, 10, None, &NEVER, |batch| {
            hits.extend(batch);
        })
        .unwrap();
        assert_eq!(hits.len(), 1, "one line is one hit");
        assert_eq!(hits[0].ranges, [0..4, 5..9, 10..14]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn crlf_lines_trim_their_terminator() {
        let dir = fresh("crlf");
        let found = grep(&dir, &[("t.txt", "alpha\r\nbeta\r\n")], "alpha", false);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].2, "alpha", "no carriage return in the preview");
        assert_eq!(found[0].1, 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_limit_cuts_and_reports_truncation() {
        let dir = fresh("limit");
        std::fs::write(dir.join("t.txt"), "m\nm\nm\nm\nm\n").unwrap();
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        let mut count = 0;
        let outcome = search(&index, "m", false, 3, None, &NEVER, |batch| {
            count += batch.len()
        })
        .unwrap();
        assert_eq!(count, 3);
        assert!(outcome.truncated);
        let outcome = search(&index, "m", false, 5, None, &NEVER, |batch| {
            count += batch.len()
        })
        .unwrap();
        assert!(!outcome.truncated);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The limit holds across the batching: a flush must not reset the
    /// count, or a monorepo-wide query runs every file to the end.
    #[test]
    fn the_limit_holds_across_flushed_batches() {
        let dir = fresh("limit-batches");
        for n in 0..10 {
            std::fs::write(dir.join(format!("f{n}.txt")), "m\nm\nm\nm\nm\nm\nm\n").unwrap();
        }
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        let mut count = 0;
        let outcome = search(&index, "m", false, 40, None, &NEVER, |batch| {
            count += batch.len()
        })
        .unwrap();
        assert_eq!(count, 40, "the limit, not the file count, ends it");
        assert!(outcome.truncated);
        // Seventy hits exist; seventy allowed takes them all, and
        // taking the last is not truncation.
        let outcome = search(&index, "m", false, 70, None, &NEVER, |batch| {
            count += batch.len()
        })
        .unwrap();
        assert!(!outcome.truncated);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn many_hits_arrive_in_batches() {
        let dir = fresh("batch");
        let text: String = (0..100).map(|n| format!("hit {n}\n")).collect();
        std::fs::write(dir.join("t.txt"), text).unwrap();
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        let mut batches = 0;
        let mut total = 0;
        search(&index, "hit", false, 500, None, &NEVER, |batch| {
            batches += 1;
            total += batch.len();
        })
        .unwrap();
        assert_eq!(total, 100);
        assert!(batches >= 3, "the answer streamed, {batches} batches");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The dirty buffer answers for the file: the disk's old text never
    /// surfaces, the buffer's new text does.
    #[test]
    fn a_buffer_override_stands_in_for_its_file() {
        let dir = fresh("buffer");
        std::fs::write(dir.join("t.rs"), "old text\n").unwrap();
        std::fs::write(dir.join("u.rs"), "old text\n").unwrap();
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        let buffer = BufferOverride {
            relative_path: Some(Arc::from("t.rs")),
            text: Arc::from("fresh words\n"),
        };
        let mut hits: Vec<ContentHit> = Vec::new();
        search(&index, "old", false, 10, Some(&buffer), &NEVER, |batch| {
            hits.extend(batch);
        })
        .unwrap();
        assert_eq!(
            hits.iter()
                .map(|h| h.relative_path.to_string())
                .collect::<Vec<_>>(),
            ["u.rs"],
            "the overridden file's disk copy never answers"
        );
        let mut hits = Vec::new();
        search(&index, "fresh", false, 10, Some(&buffer), &NEVER, |batch| {
            hits.extend(batch);
        })
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(&*hits[0].relative_path, "t.rs");
        assert_eq!(&*hits[0].line_text, "fresh words");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_superseded_query_stops_between_files() {
        let dir = fresh("cancel");
        for n in 0..50 {
            std::fs::write(dir.join(format!("f{n:02}.txt")), "hit\n").unwrap();
        }
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        let mut total = 0;
        let outcome = search(&index, "hit", false, 500, None, &|| true, |batch| {
            total += batch.len();
        })
        .unwrap();
        assert_eq!(total, 0, "nothing streamed from an abandoned run");
        assert!(!outcome.truncated);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A file that vanishes between the walk and the grep counts as
    /// skipped, not failed.
    #[test]
    fn a_missing_file_counts_skipped() {
        let dir = fresh("missing");
        std::fs::write(dir.join("here.txt"), "hit\n").unwrap();
        std::fs::write(dir.join("gone.txt"), "hit\n").unwrap();
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        std::fs::remove_file(dir.join("gone.txt")).unwrap();
        let mut total = 0;
        let outcome = search(&index, "hit", false, 10, None, &NEVER, |batch| {
            total += batch.len();
        })
        .unwrap();
        assert_eq!(total, 1);
        assert_eq!(outcome.skipped, 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
