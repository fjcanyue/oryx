//! The other occurrences of a double-clicked word, lit in the search's
//! match color while the word stays selected. The set is found once at
//! the double click and lives exactly as long as that selection.

use crate::doc::model::Document;
use crate::edit::caret::word_char;
use crate::ui::search;
use crate::ui::selection::{self, Selection};

pub struct Occurrences {
    /// The double-clicked word. The set is dropped the moment the
    /// standing selection is no longer this one.
    pub word: Selection,
    /// Every other whole-word match, in document order.
    pub matches: Vec<Selection>,
    /// The rectangles of the matches around the view, and the scroll
    /// they were computed at; the same windowing as the search's.
    pub rects: Vec<(f32, f32, f32, f32)>,
    pub rects_scroll: f32,
    /// The rectangles no longer fit the layout.
    pub stale: bool,
}

/// Whether a selected text is a word worth lighting: two characters or
/// more, every one a word character. A single letter would light half
/// the page, and a gap or a punctuation mark is not a word.
pub fn lights(text: &str) -> bool {
    text.chars().nth(1).is_some() && text.chars().all(word_char)
}

/// The occurrences of a double-clicked word, or None when the selection
/// is not a word or the word stands nowhere else.
pub fn find(doc: &Document, word: Selection) -> Option<Occurrences> {
    let text = selection::plain_text(&word, doc);
    if !lights(&text) {
        return None;
    }
    let (start, end) = word.ordered();
    // A word styled in two spans is addressed differently by the click
    // and by the search, so the clicked one is told by overlap.
    let matches: Vec<Selection> = search::word_matches(doc, &text)
        .into_iter()
        .filter(|m| {
            let (a, b) = m.ordered();
            b <= start || a >= end
        })
        .collect();
    (!matches.is_empty()).then_some(Occurrences {
        word,
        matches,
        rects: Vec::new(),
        rects_scroll: 0.0,
        stale: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::load;
    use crate::doc::markdown;
    use crate::ui::selection::{word_at, ModelPos};

    fn word(doc: &Document, block: usize, span: usize, byte: usize) -> Selection {
        word_at(doc, ModelPos { block, span, byte }).expect("a word under the click")
    }

    #[test]
    fn a_word_lights_and_a_letter_a_gap_or_a_mark_does_not() {
        assert!(lights("panel"));
        assert!(lights("snake_case"));
        assert!(lights("x2"));
        assert!(lights("été"));
        assert!(!lights("a"), "a single character");
        assert!(!lights("é"), "one character of two bytes");
        assert!(!lights("   "));
        assert!(!lights("--"));
        assert!(!lights(""));
    }

    #[test]
    fn the_other_occurrences_are_found_and_the_clicked_word_left_out() {
        let doc = markdown::parse("The panel opens.\n\nA panel, another panel and a Panel.");
        let clicked = word(&doc, 1, 0, 3);
        assert_eq!(selection::plain_text(&clicked, &doc), "panel");
        let found = find(&doc, clicked).expect("two other places");
        assert_eq!(found.word, clicked);
        let texts: Vec<String> = found
            .matches
            .iter()
            .map(|m| selection::plain_text(m, &doc))
            .collect();
        assert_eq!(texts, vec!["panel", "panel"], "exact case only");
        assert!(!found.matches.contains(&clicked));
        assert_eq!(found.matches[0].start.block, 0);
        assert!(found.stale, "the rectangles wait for the first frame");
    }

    #[test]
    fn a_word_that_stands_alone_lights_nothing() {
        let doc = markdown::parse("The panel opens.");
        assert!(find(&doc, word(&doc, 0, 0, 5)).is_none());
    }

    #[test]
    fn a_word_styled_in_two_spans_is_left_out_whole() {
        let doc = markdown::parse("A **pan**el here, a panel there.");
        let clicked = word(&doc, 0, 1, 1);
        assert_eq!(selection::plain_text(&clicked, &doc), "panel");
        let found = find(&doc, clicked).expect("the plain one");
        assert_eq!(found.matches.len(), 1);
        assert!(found.matches[0].start > clicked.end);
    }

    #[test]
    fn a_code_file_lights_whole_identifiers_only() {
        let doc = load::code_document(
            Some("rust"),
            "let count = 1;\nlet recount = count + count_all;\ncount\n",
        );
        let clicked = word(&doc, 0, 0, 5);
        assert_eq!(selection::plain_text(&clicked, &doc), "count");
        let found = find(&doc, clicked).expect("two others");
        assert_eq!(found.matches.len(), 2, "not recount, not count_all");
    }
}
