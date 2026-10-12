//! The other occurrences of the selected text, lit in the search's
//! match color while the selection stands: a double-clicked word, or a
//! text of several words on one line, selected by a drag, a Shift+click
//! or the keyboard. The set is found once per selection, when it has
//! settled, and lives exactly as long as that selection.

use crate::doc::model::Document;
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

/// The length a selection may have and still light, in characters:
/// VS Code's default limit.
const LONGEST: usize = 200;

/// Whether a selected text lights its other occurrences: two characters
/// or more, no line break, not whitespace alone, at most `LONGEST`
/// characters. A single character and a selection across lines light
/// nothing, as in VS Code and Zed.
pub fn lights(text: &str) -> bool {
    let chars = text.chars().count();
    (2..=LONGEST).contains(&chars) && !text.contains('\n') && !text.chars().all(char::is_whitespace)
}

/// The other occurrences of a selection, a word or a text on one line,
/// or None when its text does not light or stands nowhere else. The
/// match is exact, whole at an end where the text begins or ends with a
/// word character (`search::text_matches`).
pub fn find(doc: &Document, word: Selection) -> Option<Occurrences> {
    let text = selection::plain_text(&word, doc);
    if !lights(&text) {
        return None;
    }
    let (start, end) = word.ordered();
    // A word styled in two spans is addressed differently by the click
    // and by the search, so the selected one is told by overlap.
    let matches: Vec<Selection> = search::text_matches(doc, &text)
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
    fn a_word_lights_and_a_letter_or_a_gap_does_not() {
        assert!(lights("panel"));
        assert!(lights("snake_case"));
        assert!(lights("x2"));
        assert!(lights("été"));
        assert!(!lights("a"), "a single character");
        assert!(!lights("é"), "one character of two bytes");
        assert!(!lights("   "));
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
    fn a_selection_of_several_words_lights_and_a_line_break_or_spaces_do_not() {
        assert!(lights("the quick brown"));
        assert!(lights("--"), "marks light too, as in VS Code and Zed");
        assert!(lights("a."), "two characters");
        assert!(!lights("the\nquick"), "a line break");
        assert!(!lights(" \t"), "whitespace alone");
        assert!(lights(&"x".repeat(200)), "200 characters, the limit");
        assert!(!lights(&"x".repeat(201)), "over the limit");
    }

    #[test]
    fn a_sentence_lights_its_other_occurrences_whole_at_its_ends() {
        let doc = markdown::parse(
            "the quick brown fox and the quick brown dog, but the quickest cat and the quick brown.\n\nAnother the quick brown here.",
        );
        let phrase = |end: usize| Selection {
            start: ModelPos {
                block: 0,
                span: 0,
                byte: 0,
            },
            end: ModelPos {
                block: 0,
                span: 0,
                byte: end,
            },
        };
        let sel = phrase(15);
        assert_eq!(selection::plain_text(&sel, &doc), "the quick brown");
        let found = find(&doc, sel).expect("three other places");
        assert_eq!(found.word, sel);
        let texts: Vec<String> = found
            .matches
            .iter()
            .map(|m| selection::plain_text(m, &doc))
            .collect();
        assert_eq!(
            texts,
            vec!["the quick brown", "the quick brown", "the quick brown"]
        );
        assert!(
            !found.matches.contains(&sel),
            "the selection itself is left out"
        );
        let found = find(&doc, phrase(9)).expect("three other places");
        assert_eq!(found.matches.len(), 3, "\"the quickest\" is left out");
    }

    #[test]
    fn a_selection_across_paragraphs_lights_nothing() {
        let doc = markdown::parse("one two\n\none two");
        let sel = Selection {
            start: ModelPos {
                block: 0,
                span: 0,
                byte: 4,
            },
            end: ModelPos {
                block: 1,
                span: 0,
                byte: 3,
            },
        };
        assert!(selection::plain_text(&sel, &doc).contains('\n'));
        assert!(find(&doc, sel).is_none());
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
