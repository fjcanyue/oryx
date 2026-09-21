//! The fuzzy ranker: one nucleo matcher, reused across queries, scores
//! every indexed path against the query; the best files keep their
//! places and only they learn which characters they matched, so a
//! hundred-thousand-file index costs one scoring pass and a
//! highlighting pass over a handful of winners. Nothing here touches
//! the filesystem beyond the file policy's byte sniff of the few
//! top-ranked candidates whose kind a name cannot settle.

use std::sync::Arc;

use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use super::index::{IndexedFile, WorkspaceIndex};
use super::types::FileHit;
use crate::doc::load;

/// The ranker's standing state: the matcher with its scratch buffers,
/// reused for every query and every candidate.
pub struct FileMatcher {
    matcher: Matcher,
    /// The candidate being scored, as the matcher wants it.
    haystack: Vec<char>,
}

/// One candidate that scored, still pointing into the index.
struct Scored {
    score: u32,
    file: usize,
}

impl FileMatcher {
    pub fn new() -> FileMatcher {
        FileMatcher {
            matcher: Matcher::new(Config::DEFAULT),
            haystack: Vec::new(),
        }
    }

    /// Ranks the index's files against `query`, answering at most
    /// `limit` hits that Oryx can open, with each hit's matched
    /// characters as byte offsets into its path. Files rank by score,
    /// then by shorter path, then lexically, so the order is stable
    /// for equal queries. Answers whether matching files were left
    /// beyond the limit. `superseded` is polled through the scoring
    /// pass so a query the UI left behind stops costing work.
    pub fn rank(
        &mut self,
        index: &WorkspaceIndex,
        query: &str,
        limit: usize,
        superseded: &dyn Fn() -> bool,
    ) -> (Vec<FileHit>, bool) {
        let mut scored: Vec<Scored> = Vec::new();
        if query.is_empty() || limit == 0 {
            return (Vec::new(), false);
        }
        let atom = Atom::new(
            query,
            CaseMatching::Smart,
            Normalization::Never,
            AtomKind::Fuzzy,
            false,
        );
        // The first pass scores only; indices wait for the winners.
        for (at, file) in index.files().iter().enumerate() {
            if at.is_multiple_of(4096) && superseded() {
                return (Vec::new(), false);
            }
            let path = &*file.relative_path;
            let Some(score) =
                atom.score(Utf32Str::new(path, &mut self.haystack), &mut self.matcher)
            else {
                continue;
            };
            scored.push(Scored {
                score: score as u32,
                file: at,
            });
        }
        scored.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| shorter_first(index, a.file, b.file))
        });
        let mut hits = Vec::new();
        // Truncation means matching files were left beyond the limit,
        // not that undisplayable ones were skipped on the way.
        let mut truncated = false;
        for scored in scored {
            if hits.len() == limit {
                truncated = true;
                break;
            }
            let file = &index.files()[scored.file];
            // A file Oryx cannot open is not a result; the policy reads
            // bytes only for names that settle nothing, and only for
            // candidates that already ranked.
            if !load::is_displayable_file(&index.absolute(file)) {
                continue;
            }
            hits.push(self.hit(&atom, file, scored.score));
        }
        (hits, truncated)
    }

    /// One winner's hit, with the matched characters' byte offsets for
    /// highlighting.
    fn hit(&mut self, atom: &Atom, file: &IndexedFile, score: u32) -> FileHit {
        let path = &*file.relative_path;
        let mut indices: Vec<u32> = Vec::new();
        let _ = atom.indices(
            Utf32Str::new(path, &mut self.haystack),
            &mut self.matcher,
            &mut indices,
        );
        indices.sort_unstable();
        indices.dedup();
        // The matcher counts characters; the UI slices bytes.
        let mut wanted = indices.into_iter().peekable();
        let mut matched = Vec::with_capacity(wanted.len());
        for (char_no, (byte, _)) in path.char_indices().enumerate() {
            if wanted.peek() == Some(&(char_no as u32)) {
                wanted.next();
                matched.push(byte as u32);
            }
        }
        FileHit {
            relative_path: Arc::from(&*file.relative_path),
            basename_start: file.basename_start as u32,
            score,
            matched,
        }
    }
}

/// Equal scores order by the shorter path, then lexically, so the
/// ranking never shuffles between identical queries.
fn shorter_first(index: &WorkspaceIndex, a: usize, b: usize) -> std::cmp::Ordering {
    let (x, y) = (&index.files()[a], &index.files()[b]);
    x.relative_path
        .len()
        .cmp(&y.relative_path.len())
        .then_with(|| x.relative_path.cmp(&y.relative_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_search::types::RootGeneration;

    const NEVER: fn() -> bool = || false;

    /// An index over paths named outright, since matching never reads
    /// the files; the display policy sees known extensions. Each call
    /// takes its own folder, so parallel tests never sweep one
    /// another's.
    fn index_over(paths: &[&str]) -> WorkspaceIndex {
        static CALL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "oryx-matcher-{}-{}",
            std::process::id(),
            CALL.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for path in paths {
            if let Some(parent) = std::path::Path::new(path).parent() {
                std::fs::create_dir_all(dir.join(parent)).unwrap();
            }
            std::fs::write(dir.join(path), "x").unwrap();
        }
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        index
    }

    fn ranked(matcher: &mut FileMatcher, index: &WorkspaceIndex, query: &str) -> Vec<String> {
        matcher
            .rank(index, query, 100, &NEVER)
            .0
            .iter()
            .map(|hit| hit.relative_path.to_string())
            .collect()
    }

    #[test]
    fn an_exact_name_scores_above_a_partial_one() {
        let index = index_over(&["src/main.rs", "main.rs", "README.md"]);
        let mut matcher = FileMatcher::new();
        assert_eq!(
            ranked(&mut matcher, &index, "main.rs"),
            ["main.rs", "src/main.rs"]
        );
    }

    #[test]
    fn a_partial_name_finds_its_file() {
        let index = index_over(&["UserController.rs", "UserService.rs", "notes.md"]);
        let mut matcher = FileMatcher::new();
        let found = ranked(&mut matcher, &index, "usercon");
        assert_eq!(found, ["UserController.rs"]);
    }

    #[test]
    fn a_fuzzy_query_matches_across_the_path() {
        let index = index_over(&["src/user/UserController.rs", "docs/guide.md"]);
        let mut matcher = FileMatcher::new();
        let found = ranked(&mut matcher, &index, "usrctrl");
        assert_eq!(found, ["src/user/UserController.rs"]);
    }

    #[test]
    fn a_nested_path_ranks_by_score_then_shortness_then_lexical() {
        let index = index_over(&[
            "a/match.rs",
            "match.rs",
            "b/match.rs",
            "deep/longer/path/match.rs",
        ]);
        let mut matcher = FileMatcher::new();
        assert_eq!(
            ranked(&mut matcher, &index, "match"),
            [
                "match.rs",
                "a/match.rs",
                "b/match.rs",
                "deep/longer/path/match.rs"
            ],
            "the shallow paths share a score and order lexically"
        );
    }

    #[test]
    fn smart_case_matches_insensitively_until_a_capital_is_typed() {
        // The two files differ by a capital that a case-blind folder
        // cannot hold side by side, so one sits a level down.
        let index = index_over(&["user.rs", "deep/User.rs"]);
        let mut matcher = FileMatcher::new();
        assert_eq!(ranked(&mut matcher, &index, "user").len(), 2);
        assert_eq!(ranked(&mut matcher, &index, "User"), ["deep/User.rs"]);
    }

    #[test]
    fn unicode_and_chinese_names_match() {
        let index = index_over(&["笔记.md", "src/カタログ.rs", "notas.txt"]);
        let mut matcher = FileMatcher::new();
        assert_eq!(ranked(&mut matcher, &index, "笔记"), ["笔记.md"]);
        assert_eq!(
            ranked(&mut matcher, &index, "カタログ"),
            ["src/カタログ.rs"]
        );
    }

    #[test]
    fn a_query_no_path_answers_needs_empty() {
        let index = index_over(&["alpha.rs", "beta.md"]);
        let mut matcher = FileMatcher::new();
        let (hits, truncated) = matcher.rank(&index, "zzzzz", 100, &NEVER);
        assert!(hits.is_empty());
        assert!(!truncated);
    }

    #[test]
    fn an_empty_query_answers_nothing() {
        let index = index_over(&["alpha.rs"]);
        let mut matcher = FileMatcher::new();
        let (hits, truncated) = matcher.rank(&index, "", 100, &NEVER);
        assert!(hits.is_empty());
        assert!(!truncated);
    }

    #[test]
    fn the_limit_cuts_and_reports_truncation() {
        let index = index_over(&["a/match.rs", "b/match.rs", "c/match.rs"]);
        let mut matcher = FileMatcher::new();
        let (hits, truncated) = matcher.rank(&index, "match", 2, &NEVER);
        assert_eq!(hits.len(), 2);
        assert!(truncated);
        let (all, truncated) = matcher.rank(&index, "match", 3, &NEVER);
        assert_eq!(all.len(), 3);
        assert!(!truncated);
    }

    #[test]
    fn equal_queries_answer_in_the_same_order() {
        let paths: Vec<String> = (0..50).map(|n| format!("mod{n:02}.rs")).collect();
        let named: Vec<&str> = paths.iter().map(String::as_str).collect();
        let index = index_over(&named);
        let mut matcher = FileMatcher::new();
        let first = ranked(&mut matcher, &index, "mod");
        let second = ranked(&mut matcher, &index, "mod");
        assert_eq!(first, second);
        assert_eq!(first.len(), 50);
    }

    #[test]
    fn matched_offsets_land_on_the_matched_characters() {
        let index = index_over(&["main.rs"]);
        let mut matcher = FileMatcher::new();
        let (hits, _) = matcher.rank(&index, "mrs", 10, &NEVER);
        let hit = &hits[0];
        for byte in &hit.matched {
            let text = &hit.relative_path[*byte as usize..];
            assert!(
                text.starts_with('m') || text.starts_with('r') || text.starts_with('s'),
                "offset {byte} points at {}",
                text.chars().next().unwrap_or(' ')
            );
        }
        assert!(hit
            .matched
            .iter()
            .any(|b| hit.relative_path[*b as usize..].starts_with('m')));
        assert!(
            hit.matched
                .iter()
                .any(|b| hit.relative_path[*b as usize..].starts_with('s')),
            "the last character of the query matched"
        );
    }

    /// A file the policy refuses — a PDF by name — never becomes a
    /// result, even when it out-scores the rest.
    #[test]
    fn an_undisplayable_file_is_not_a_result() {
        let index = index_over(&["guide.pdf", "guide.rs"]);
        let mut matcher = FileMatcher::new();
        assert_eq!(ranked(&mut matcher, &index, "guide"), ["guide.rs"]);
    }

    /// Scoring stops when the query is superseded, mid-pass.
    #[test]
    fn a_superseded_scoring_pass_stops() {
        let paths: Vec<String> = (0..8192).map(|n| format!("f{n:04}.rs")).collect();
        let named: Vec<&str> = paths.iter().map(String::as_str).collect();
        let index = index_over(&named);
        let mut matcher = FileMatcher::new();
        let (hits, _) = matcher.rank(&index, "f", 100, &|| true);
        assert!(hits.is_empty(), "the pass abandoned itself");
    }
}
