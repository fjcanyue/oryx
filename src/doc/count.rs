//! Word, character and line counts of a file's text, taken the way the
//! page reads: markdown marks, link addresses, HTML tags and frontmatter
//! are not text, so they are not counted.
//!
//! A word is a run of characters between spaces that holds a letter or a
//! digit, so `don't`, `well-known` and `3.14` are one word each and a
//! lone dash is none. Chinese, Japanese and Korean characters count one
//! word each. Characters count spaces and leave line breaks out. A line
//! is a line of text as the page shows it before the window wraps it: a
//! hard break starts one, a soft break does not, and a line with no text
//! on it is not counted. A code file counts every line it has.

use std::sync::Arc;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use crate::doc::load::FileKind;
use crate::doc::markdown::options;

/// Adult silent reading of non-fiction, words a minute (Brysbaert 2019,
/// a meta-analysis of 190 studies, finds 238).
const WORDS_A_MINUTE: usize = 240;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub words: usize,
    pub chars: usize,
    pub lines: usize,
}

/// Which figures a kind of file shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Words, characters, lines and reading time.
    Prose,
    /// Lines and characters: words and minutes say nothing about code.
    Code,
}

/// The figures a kind of file shows, None for books and comics.
pub fn scope(kind: FileKind) -> Option<Scope> {
    match kind {
        FileKind::Markdown | FileKind::Text => Some(Scope::Prose),
        FileKind::Code(_) | FileKind::Unknown => Some(Scope::Code),
        FileKind::Epub
        | FileKind::Fb2
        | FileKind::Kindle
        | FileKind::Comic
        | FileKind::Undisplayable => None,
    }
}

/// Counts a markdown source as its rendered page reads.
pub fn markdown(source: &str) -> Counts {
    let mut tally = Tally::default();
    let mut html = Html::Text;
    // Inside an image's alt text or the frontmatter, which the page
    // does not show as text.
    let mut hidden = 0usize;
    for event in Parser::new_ext(source, options()) {
        match event {
            Event::Start(Tag::Image { .. } | Tag::MetadataBlock(_)) => hidden += 1,
            Event::End(TagEnd::Image | TagEnd::MetadataBlock(_)) => {
                hidden = hidden.saturating_sub(1)
            }
            _ if hidden > 0 => {}
            Event::Text(text) | Event::Code(text) => tally.push(&text),
            Event::InlineMath(_) | Event::DisplayMath(_) => tally.formula(),
            Event::SoftBreak => tally.push(" "),
            Event::HardBreak => tally.end_line(),
            Event::Html(raw) => html.push(&raw, true, &mut tally),
            Event::InlineHtml(raw) => html.push(&raw, false, &mut tally),
            // Marks inside a line: the word and the line run through.
            Event::Start(
                Tag::Emphasis
                | Tag::Strong
                | Tag::Strikethrough
                | Tag::Superscript
                | Tag::Subscript
                | Tag::Link { .. },
            )
            | Event::End(
                TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Superscript
                | TagEnd::Subscript
                | TagEnd::Link,
            ) => {}
            // The cells of a row share its line.
            Event::Start(Tag::TableCell) | Event::End(TagEnd::TableCell) => tally.end_word(),
            Event::Start(_) | Event::End(_) => tally.end_line(),
            Event::FootnoteReference(_) | Event::TaskListMarker(_) | Event::Rule => {}
        }
    }
    tally.finish()
}

/// Counts text that is already what the reader sees: a text file, or
/// the plain text of a selection on the rendered page.
pub fn text(text: &str) -> Counts {
    let mut tally = Tally::default();
    tally.push(text);
    tally.finish()
}

/// Counts a code file: every line and every character, no words.
pub fn code(text: &str) -> Counts {
    Counts {
        words: 0,
        chars: text.chars().filter(|c| !matches!(c, '\n' | '\r')).count(),
        lines: text.lines().count(),
    }
}

/// The counts the corner shows for an open file: the selection's when
/// text is selected, the whole file's otherwise. `selected` is the
/// selection's plain text, which in the editor is a slice of the source
/// and on the rendered page is already what the reader sees.
pub fn of_file(
    kind: FileKind,
    editing: bool,
    source: &str,
    selected: Option<&str>,
) -> Option<(Counts, Scope)> {
    let scope = scope(kind)?;
    let marked = matches!(kind, FileKind::Markdown);
    let counts = match (scope, selected) {
        (Scope::Code, _) => code(selected.unwrap_or(source)),
        (Scope::Prose, None) if marked => markdown(source),
        (Scope::Prose, Some(slice)) if marked && editing => markdown(slice),
        (Scope::Prose, _) => text(selected.unwrap_or(source)),
    };
    Some((counts, scope))
}

/// One count, owning what it reads so it can run off the window's
/// thread: the 8 MB markdown tier takes 140 ms to count.
pub struct Job {
    pub kind: FileKind,
    pub editing: bool,
    pub source: Arc<str>,
    pub selected: Option<String>,
}

impl Job {
    /// The corner's text, None for a kind that shows no count.
    pub fn line(&self) -> Option<String> {
        of_file(
            self.kind,
            self.editing,
            &self.source,
            self.selected.as_deref(),
        )
        .map(|(counts, scope)| line(&counts, scope))
    }
}

/// Reading time in whole minutes, rounded up; zero only for no words.
pub fn minutes(words: usize) -> usize {
    words.div_ceil(WORDS_A_MINUTE)
}

/// Reading time as the corner shows it: minutes, and hours past the
/// first, so a manuscript reads "6 h 15 min" and not "375 min".
pub fn reading_time(words: usize) -> String {
    let total = minutes(words);
    match (total / 60, total % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{} h", grouped(h)),
        (h, m) => format!("{} h {m} min", grouped(h)),
    }
}

/// The corner's text for these counts.
pub fn line(counts: &Counts, scope: Scope) -> String {
    let figure = |n: usize, one: &str, many: &str| {
        format!("{} {}", grouped(n), if n == 1 { one } else { many })
    };
    let words = figure(counts.words, "word", "words");
    let chars = figure(counts.chars, "character", "characters");
    let lines = figure(counts.lines, "line", "lines");
    match scope {
        Scope::Prose => format!(
            "{words} \u{00B7} {chars} \u{00B7} {lines} \u{00B7} {}",
            reading_time(counts.words)
        ),
        Scope::Code => format!("{lines} \u{00B7} {chars}"),
    }
}

/// A number with a comma between its thousands.
fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, d) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(d);
    }
    out
}

/// A script written without spaces between words, counted a character
/// a word: Han, the kana and Hangul.
fn by_character(c: char) -> bool {
    matches!(
        c as u32,
        0x3040..=0x30FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xAC00..=0xD7AF
            | 0xF900..=0xFAFF
            | 0x20000..=0x323AF
    )
}

/// The running count. Text arrives in pieces (a word split by a bold
/// mark is two pieces), so the open word and the open line carry over
/// from one piece to the next.
#[derive(Default)]
struct Tally {
    counts: Counts,
    /// The run since the last space holds a letter or a digit.
    word: bool,
    /// Text has landed on the current line.
    line: bool,
}

impl Tally {
    fn push(&mut self, text: &str) {
        for c in text.chars() {
            self.push_char(c);
        }
    }

    fn push_char(&mut self, c: char) {
        if matches!(c, '\n' | '\r') {
            self.end_line();
            return;
        }
        self.counts.chars += 1;
        if c.is_whitespace() {
            self.end_word();
            return;
        }
        self.open_line();
        if by_character(c) {
            self.end_word();
            self.counts.words += 1;
        } else if c.is_alphanumeric() {
            self.word = true;
        }
    }

    /// A typeset formula: one word on its line, its source not read.
    fn formula(&mut self) {
        self.end_word();
        self.open_line();
        self.counts.words += 1;
    }

    fn open_line(&mut self) {
        if !self.line {
            self.line = true;
            self.counts.lines += 1;
        }
    }

    fn end_word(&mut self) {
        if self.word {
            self.word = false;
            self.counts.words += 1;
        }
    }

    fn end_line(&mut self) {
        self.end_word();
        self.line = false;
    }

    fn finish(mut self) -> Counts {
        self.end_word();
        self.counts
    }
}

/// Where a run of raw HTML stands, kept across events since a tag or a
/// comment may span several of them.
enum Html {
    Text,
    /// Inside a tag: its name so far, whether the name is still being
    /// read, and the quote an attribute value opened.
    Tag {
        name: String,
        naming: bool,
        quote: Option<char>,
    },
    /// Inside a comment, with the dashes seen in a row.
    Comment(u8),
}

impl Html {
    /// Feeds raw HTML: the text between tags is counted, the tags and
    /// comments are not. A `<br>` ends the line; any other tag ends the
    /// word in an HTML block and lets it run through inside a line.
    fn push(&mut self, raw: &str, block: bool, tally: &mut Tally) {
        let mut rest = raw.chars();
        while let Some(c) = rest.next() {
            match self {
                Html::Text if c == '<' => {
                    *self = if rest.as_str().starts_with("!--") {
                        Html::Comment(0)
                    } else {
                        Html::Tag {
                            name: String::new(),
                            naming: true,
                            quote: None,
                        }
                    };
                }
                Html::Text => tally.push_char(c),
                Html::Tag {
                    name,
                    naming,
                    quote,
                } => match (*quote, c) {
                    (Some(q), _) if c == q => *quote = None,
                    (Some(_), _) => {}
                    (None, '"' | '\'') => *quote = Some(c),
                    (None, '>') => {
                        if name.eq_ignore_ascii_case("br") {
                            tally.end_line();
                        } else if block {
                            tally.end_word();
                        }
                        *self = Html::Text;
                    }
                    (None, '/') if name.is_empty() => {}
                    (None, _) if *naming && c.is_ascii_alphanumeric() => name.push(c),
                    (None, _) => *naming = false,
                },
                Html::Comment(dashes) => match c {
                    '-' => *dashes = dashes.saturating_add(1),
                    '>' if *dashes >= 2 => *self = Html::Text,
                    _ => *dashes = 0,
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(words: usize, chars: usize, lines: usize) -> Counts {
        Counts {
            words,
            chars,
            lines,
        }
    }

    #[test]
    fn a_word_is_a_spaced_run_holding_a_letter_or_digit() {
        let c = text("don't stop well-known 3.14 1,234 \u{2014} e.g.");
        assert_eq!(c.words, 6, "the lone dash is not a word");
    }

    #[test]
    fn markdown_marks_and_link_addresses_are_not_counted() {
        let c = markdown("# Title\n\n- item with `code`\n- **bo**ld [link](https://a.tld/page)\n");
        // "Title", "item with code", "bold link"
        assert_eq!(c, counts(6, 28, 3));
    }

    #[test]
    fn frontmatter_is_not_counted() {
        let c = markdown("---\ntitle: Hello world\n---\n\nBody text.\n");
        assert_eq!(c, counts(2, 10, 1));
    }

    #[test]
    fn a_code_block_counts_its_lines_that_hold_text() {
        let c = markdown("Intro.\n\n```rust\nlet a = 1;\n\nlet b = 2;\n```\n");
        assert_eq!(c, counts(7, 26, 3));
    }

    #[test]
    fn a_hard_break_starts_a_line_and_a_soft_break_does_not() {
        let c = markdown("one  \ntwo\\\nthree\nfour<br>five\n");
        // "one", "two", "three four", "five"
        assert_eq!(c, counts(5, 20, 4));
    }

    #[test]
    fn a_poem_counts_its_verses_only_with_hard_breaks() {
        let broken = markdown("Roses are red,  \nviolets are blue,  \nsugar is sweet.\n");
        assert_eq!(broken.lines, 3);
        let joined = markdown("Roses are red,\nviolets are blue,\nsugar is sweet.\n");
        assert_eq!(joined.lines, 1, "soft breaks read as spaces");
        assert_eq!(joined.words, broken.words);
    }

    #[test]
    fn a_table_row_is_one_line() {
        let c = markdown("| a | b |\n|---|---|\n| c d | e |\n");
        assert_eq!(c, counts(5, 6, 2));
    }

    #[test]
    fn a_tight_and_a_loose_list_count_an_item_once() {
        assert_eq!(markdown("- a\n- b\n").lines, 2);
        assert_eq!(markdown("- a\n\n- b\n").lines, 2);
        assert_eq!(markdown("- a\n  - b\n- c\n").lines, 3);
    }

    #[test]
    fn chinese_japanese_and_korean_count_by_character() {
        assert_eq!(
            text("\u{4F60}\u{597D}\u{4E16}\u{754C} hello"),
            counts(5, 10, 1)
        );
        assert_eq!(text("\u{3053}\u{3093}\u{306B}\u{3061}\u{306F}").words, 5);
        assert_eq!(text("\u{C548}\u{B155}\u{D558}\u{C138}\u{C694}").words, 5);
    }

    #[test]
    fn an_image_and_a_footnote_mark_are_not_words() {
        let c = markdown("See ![alt text](p.png) here[^1].\n\n[^1]: The note.\n");
        assert_eq!(c.words, 4);
        assert_eq!(c.lines, 2);
    }

    #[test]
    fn an_html_block_counts_its_text_without_the_tags() {
        let c = markdown("<div align=\"right\">\nmore tips here\n</div>\n");
        assert_eq!(c, counts(3, 14, 1));
        let c = markdown("<!-- a note > to self -->\n\nShown.\n");
        assert_eq!(c, counts(1, 6, 1));
    }

    #[test]
    fn a_formula_counts_as_one_word() {
        let c = markdown("Energy $E = mc^2$ here.\n");
        assert_eq!(c.words, 3);
        assert_eq!(c.lines, 1);
    }

    #[test]
    fn a_text_file_counts_every_line_that_holds_text() {
        assert_eq!(text("first line\n\nsecond\n"), counts(3, 16, 2));
        assert_eq!(text("a b\r\nc\r\n"), counts(3, 4, 2));
        assert_eq!(text(""), counts(0, 0, 0));
    }

    #[test]
    fn a_code_file_counts_every_line_and_no_words() {
        assert_eq!(code("fn main() {\n\n}\n"), counts(0, 12, 3));
        assert_eq!(code("a\r\nb"), counts(0, 2, 2));
        assert_eq!(code(""), counts(0, 0, 0));
    }

    #[test]
    fn reading_time_rounds_up_at_240_words_a_minute() {
        assert_eq!(minutes(0), 0);
        assert_eq!(minutes(1), 1);
        assert_eq!(minutes(240), 1);
        assert_eq!(minutes(241), 2);
        assert_eq!(minutes(1200), 5);
    }

    #[test]
    fn the_line_names_each_figure() {
        assert_eq!(
            line(&counts(1234, 6910, 87), Scope::Prose),
            "1,234 words \u{00B7} 6,910 characters \u{00B7} 87 lines \u{00B7} 6 min"
        );
        assert_eq!(
            line(&counts(1, 1, 1), Scope::Prose),
            "1 word \u{00B7} 1 character \u{00B7} 1 line \u{00B7} 1 min"
        );
        assert_eq!(
            line(&counts(0, 1_234_567, 87), Scope::Code),
            "87 lines \u{00B7} 1,234,567 characters"
        );
    }

    #[test]
    fn reading_time_past_an_hour_reads_in_hours() {
        assert_eq!(reading_time(59 * 240), "59 min");
        assert_eq!(reading_time(60 * 240), "1 h");
        assert_eq!(reading_time(90_000), "6 h 15 min");
        assert!(line(&counts(90_000, 1, 1), Scope::Prose).ends_with("\u{00B7} 6 h 15 min"));
    }

    #[test]
    fn a_job_gives_the_corner_its_line() {
        let job = |kind, selected: Option<&str>| Job {
            kind,
            editing: false,
            source: Arc::from("Two words.\n"),
            selected: selected.map(str::to_string),
        };
        assert_eq!(
            job(FileKind::Markdown, None).line().as_deref(),
            Some("2 words \u{00B7} 10 characters \u{00B7} 1 line \u{00B7} 1 min")
        );
        assert_eq!(
            job(FileKind::Markdown, Some("Two")).line().as_deref(),
            Some("1 word \u{00B7} 3 characters \u{00B7} 1 line \u{00B7} 1 min")
        );
        assert_eq!(
            job(FileKind::Unknown, None).line().as_deref(),
            Some("1 line \u{00B7} 10 characters")
        );
        assert_eq!(job(FileKind::Epub, None).line(), None);
    }

    #[test]
    fn prose_kinds_get_words_and_the_rest_lines_and_characters() {
        assert_eq!(scope(FileKind::Markdown), Some(Scope::Prose));
        assert_eq!(scope(FileKind::Text), Some(Scope::Prose));
        assert_eq!(scope(FileKind::Code("rust")), Some(Scope::Code));
        assert_eq!(scope(FileKind::Unknown), Some(Scope::Code));
        for book in [
            FileKind::Epub,
            FileKind::Fb2,
            FileKind::Kindle,
            FileKind::Comic,
            FileKind::Undisplayable,
        ] {
            assert_eq!(scope(book), None);
        }
    }

    #[test]
    fn the_whole_file_counts_the_same_on_both_surfaces() {
        let source = "# Title\n\nSome **bold** text.\n";
        let reading = of_file(FileKind::Markdown, false, source, None);
        let editing = of_file(FileKind::Markdown, true, source, None);
        assert_eq!(reading, Some((counts(4, 20, 2), Scope::Prose)));
        assert_eq!(editing, reading);
    }

    #[test]
    fn a_selection_shows_its_own_counts() {
        let source = "# Title\n\nSome **bold** text.\n";
        // In the editor the selection is a slice of the source.
        let editing = of_file(FileKind::Markdown, true, source, Some("**bold** text"));
        assert_eq!(editing, Some((counts(2, 9, 1), Scope::Prose)));
        // On the page it is already what the reader sees, and a star
        // the reader sees is a character.
        let reading = of_file(FileKind::Markdown, false, source, Some("2 * 3"));
        assert_eq!(reading, Some((counts(2, 5, 1), Scope::Prose)));
        let code = of_file(FileKind::Code("rust"), true, "a\nb\nc\n", Some("a\nb"));
        assert_eq!(code, Some((counts(0, 2, 2), Scope::Code)));
        assert_eq!(of_file(FileKind::Epub, false, "text", None), None);
    }
}
