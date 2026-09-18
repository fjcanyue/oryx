//! Find in document: smart-case substring matching over the document's
//! display text. Each match is a `Selection`, so highlight geometry and
//! scroll targets reuse the selection machinery.

use std::borrow::Cow;
use std::ops::Range;

use crate::doc::model::Document;
use crate::paint::painter::Painter;
use crate::style::fonts::CODE_FAMILY;
use crate::style::theme::{Rgba, Theme};
use crate::ui::selection::{block_pieces, ModelPos, Piece, Selection};
use crate::ui::textfield::TextField;

/// A character no query can contain, planted between blocks so a match
/// never crosses them.
const BLOCK_SEP: char = '\u{1}';

const BAR_WIDTH: f32 = 344.0;
pub(crate) const BAR_HEIGHT: f32 = 40.0;
pub(crate) const MARGIN: f32 = 16.0;
pub(crate) const PAD: f32 = 16.0;
pub(crate) const RADIUS: f32 = 20.0;
pub(crate) const QUERY_SIZE: f32 = 15.0;
pub(crate) const COUNTER_SIZE: f32 = 13.0;
const TOGGLE_W: f32 = 26.0;
const TOGGLE_H: f32 = 22.0;
const ROW_H: f32 = 36.0;
/// The glyphs sit this much below the row's top, so the text's caps
/// stand centered on the caret and the pill, level with the toggle.
pub(crate) const TEXT_DROP: f32 = 2.0;

/// Live find session: the query as typed, its matches, and the cursor
/// among them. `stale` marks the matches for recomputation against the
/// current layout on the next frame; `rects` caches every match's
/// highlight boxes with their match index, so painting a frame never
/// re-shapes text. The query is a `TextField`, so reopening the bar with
/// the old query selected is a plain `select_all` and the next typed
/// character replaces it.
pub struct SearchState {
    pub query: TextField,
    /// Where the query's text was last drawn, for the mouse.
    pub query_view: FieldView,
    /// The same for the replace field.
    pub replace_view: FieldView,
    /// Regex matching instead of plain text, flipped by the bar's
    /// toggle and kept across close and reopen like the query.
    pub regex: bool,
    /// The pattern failed to compile or exceeded the backtracking
    /// limit; the bar shows the caution border and no counter.
    pub error: bool,
    /// The replace row while find and replace is open; edit mode only,
    /// dropped on leaving the editor. Focus lives inside the row, so a
    /// focused replace field without a row cannot exist.
    pub replace: Option<ReplaceRow>,
    /// A replace happened at this source offset: once the matches
    /// recompute, the current one seats on the first at or after it,
    /// wrapping to the first. Consumed by the recompute, however many
    /// frames the relayout takes to let it run.
    pub seek: Option<usize>,
    /// A replace just edited the document, so Ctrl+Z and Ctrl+Shift+Z
    /// pass through to it and take the replacement back. Any key the
    /// bar claims turns them back to the field. Clicks never set this:
    /// a click off the bar closes it instead.
    pub doc_intent: bool,
    pub matches: Vec<Selection>,
    /// Highlight rects for the matches inside the band window alone;
    /// the counter and navigation use the full match list.
    pub rects: Vec<(usize, (f32, f32, f32, f32))>,
    /// The scroll position `rects` was computed around, so drifting a
    /// viewport past it triggers a refresh.
    pub rects_scroll: f32,
    pub current: usize,
    pub stale: bool,
    /// The current match landed on its recorded block top because its
    /// region was cold; the exact anchor still owes a centering.
    pub settle: bool,
}

/// Where a field's text was last drawn: the left edge of its text and
/// the x of every character boundary from there, the windowed text's
/// characters at their drawn places and the cut ones beyond the box. A
/// click and a drag read it; the bar refreshes it every frame it
/// draws, so it describes the text the pointer sees.
#[derive(Debug, Default, Clone)]
pub struct FieldView {
    pub left: f32,
    pub offsets: Vec<f32>,
}

/// The second row of the bar: the replacement text and whether its
/// field takes the typing instead of the query.
pub struct ReplaceRow {
    pub field: TextField,
    pub focused: bool,
}

impl SearchState {
    /// The replace row is shown and takes the typing.
    pub fn replace_focused(&self) -> bool {
        self.replace.as_ref().is_some_and(|row| row.focused)
    }

    /// The field the keyboard feeds.
    pub fn focused(&self) -> &TextField {
        match self.replace.as_ref() {
            Some(row) if row.focused => &row.field,
            _ => &self.query,
        }
    }

    pub fn focused_mut(&mut self) -> &mut TextField {
        match self.replace.as_mut() {
            Some(row) if row.focused => &mut row.field,
            _ => &mut self.query,
        }
    }

    /// The focused field's selected text, when any; the one accessor
    /// copy and cut share so they cannot drift.
    pub fn focused_selection(&self) -> Option<String> {
        let text = self.focused().selected_text();
        (!text.is_empty()).then(|| text.to_string())
    }
}

/// The floating pill over the document's top-right corner: query on the
/// left, match counter on the right, in the theme's overlay colors. The
/// fields' views are refreshed for the mouse.
pub fn draw_bar(painter: &mut Painter, theme: &Theme, state: &mut SearchState, width: f32) {
    let ui = &theme.ui;
    let x = (width - BAR_WIDTH - MARGIN).max(MARGIN);
    let y = MARGIN;
    let height = if state.replace.is_some() {
        BAR_HEIGHT + ROW_H
    } else {
        BAR_HEIGHT
    };
    for (grow, alpha) in [(6.0, 16), (3.0, 28)] {
        painter.fill(
            x - grow,
            y - grow + 1.5,
            BAR_WIDTH + 2.0 * grow,
            height + 2.0 * grow,
            RADIUS + grow,
            Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: alpha,
            },
        );
    }
    painter.fill(x, y, BAR_WIDTH, height, RADIUS, ui.overlay_bg);
    let border = if state.error {
        theme.alerts.caution
    } else {
        theme.blocks.table_border
    };
    painter.stroke(x, y, BAR_WIDTH, height, RADIUS, 1.0, border);

    // The regex toggle, drawn on the selection background while active.
    let toggle_x = x + BAR_WIDTH - 10.0 - TOGGLE_W;
    let toggle_y = y + (BAR_HEIGHT - TOGGLE_H) / 2.0;
    if state.regex {
        painter.fill(toggle_x, toggle_y, TOGGLE_W, TOGGLE_H, 6.0, ui.selection_bg);
    }
    let label_w = painter.measure(".*", CODE_FAMILY, COUNTER_SIZE, 600);
    let label_color = if state.regex {
        ui.overlay_fg
    } else {
        theme.blocks.frontmatter_fg
    };
    painter.text(
        toggle_x + (TOGGLE_W - label_w) / 2.0,
        toggle_y + 2.0,
        ".*",
        CODE_FAMILY,
        COUNTER_SIZE,
        600,
        label_color,
    );

    let counter = if state.query.is_empty() || state.error {
        String::new()
    } else {
        let shown = if state.matches.is_empty() {
            0
        } else {
            state.current + 1
        };
        format!("{shown}/{}", state.matches.len())
    };
    let counter_w = painter.measure(&counter, CODE_FAMILY, COUNTER_SIZE, 400);
    let counter_color = if !state.query.is_empty() && state.matches.is_empty() {
        theme.alerts.caution
    } else {
        theme.blocks.frontmatter_fg
    };
    // Tops differ by the ascent difference so both texts share a baseline.
    painter.text(
        toggle_x - 10.0 - counter_w,
        y + 10.0 + TEXT_DROP + (QUERY_SIZE - COUNTER_SIZE) * 1.1,
        &counter,
        CODE_FAMILY,
        COUNTER_SIZE,
        400,
        counter_color,
    );

    let avail = toggle_x - 10.0 - counter_w - 12.0 - (x + PAD);
    state.query_view = draw_field(
        painter,
        theme,
        &state.query,
        x + PAD,
        y + 10.0,
        avail,
        "find",
        !state.replace_focused(),
    );

    if let Some(row) = state.replace.as_ref() {
        painter.line(
            x + PAD,
            y + BAR_HEIGHT - 1.0,
            x + BAR_WIDTH - PAD,
            y + BAR_HEIGHT - 1.0,
            1.0,
            theme.blocks.table_border,
        );
        // The replace-all hint sits where the counter does above, in
        // the same face and color.
        let hint = "Ctrl+\u{23CE} all";
        let hint_w = painter.measure(hint, CODE_FAMILY, COUNTER_SIZE, 400);
        painter.text(
            x + BAR_WIDTH - PAD - hint_w,
            y + BAR_HEIGHT + 7.0 + TEXT_DROP + (QUERY_SIZE - COUNTER_SIZE) * 1.1,
            hint,
            CODE_FAMILY,
            COUNTER_SIZE,
            400,
            theme.blocks.frontmatter_fg,
        );
        state.replace_view = draw_field(
            painter,
            theme,
            &row.field,
            x + PAD,
            y + BAR_HEIGHT + 7.0,
            BAR_WIDTH - 2.0 * PAD - hint_w - 12.0,
            "replace",
            row.focused,
        );
    }
}

/// What a click at logical window coordinates lands on in the bar.
pub enum BarHit {
    Toggle,
    Query,
    Replace,
}

/// The bar element under a point, `None` outside the pill; the geometry
/// mirrors `draw_bar`.
pub fn bar_hit(width: f32, replace_row: bool, px: f32, py: f32) -> Option<BarHit> {
    let x = (width - BAR_WIDTH - MARGIN).max(MARGIN);
    let height = if replace_row {
        BAR_HEIGHT + ROW_H
    } else {
        BAR_HEIGHT
    };
    if !(x..x + BAR_WIDTH).contains(&px) || !(MARGIN..MARGIN + height).contains(&py) {
        return None;
    }
    if toggle_hit(width, px, py) {
        return Some(BarHit::Toggle);
    }
    if replace_row && py >= MARGIN + BAR_HEIGHT {
        return Some(BarHit::Replace);
    }
    Some(BarHit::Query)
}

/// One field row: windowed text with the caret kept visible, the
/// placeholder when empty, selection and caret drawn only on the field
/// the keyboard feeds. Answers where the text stands, for the mouse.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_field(
    painter: &mut Painter,
    theme: &Theme,
    field: &TextField,
    left: f32,
    top: f32,
    avail: f32,
    placeholder: &str,
    focused: bool,
) -> FieldView {
    let ui = &theme.ui;
    let text = field.text();
    let (window, cut) = window_fit(painter, text, field.caret(), avail);
    let shown = if cut {
        format!("\u{2026}{}", &text[window.clone()])
    } else {
        text[window.clone()].to_string()
    };
    let lead = if cut {
        painter.measure("\u{2026}", CODE_FAMILY, QUERY_SIZE, 400)
    } else {
        0.0
    };
    // Offsets are taken inside the drawn window, so a caret in the middle
    // of a text too long for the pill still lands under its character.
    let x_of = |painter: &mut Painter, at: usize| {
        let at = at.clamp(window.start, window.end);
        lead + painter.measure(&text[window.start..at], CODE_FAMILY, QUERY_SIZE, 400)
    };
    if focused {
        if let Some(range) = field.selection() {
            let from = x_of(painter, range.start);
            let to = x_of(painter, range.end);
            painter.fill(
                left + from - 2.0,
                top - 1.0,
                to - from + 4.0,
                BAR_HEIGHT - 18.0,
                4.0,
                ui.selection_bg,
            );
        }
    }
    if text.is_empty() {
        painter.text(
            left + 6.0,
            top + TEXT_DROP,
            placeholder,
            CODE_FAMILY,
            QUERY_SIZE,
            400,
            theme.blocks.frontmatter_fg,
        );
    } else {
        painter.text(
            left,
            top + TEXT_DROP,
            &shown,
            CODE_FAMILY,
            QUERY_SIZE,
            400,
            ui.overlay_fg,
        );
    }
    if focused {
        let caret_x = left + x_of(painter, field.caret()) + 1.0;
        painter.line(caret_x, top + 1.0, caret_x, top + 19.0, 1.0, ui.overlay_fg);
    }
    // Every boundary from the drawn window's start, the cut ones
    // before it at negative x: a click at the box's left edge reaches
    // them one at a time, and the window follows the caret.
    let offsets = field
        .boundaries()
        .map(|at| {
            if at >= window.start {
                lead + painter.measure(&text[window.start..at], CODE_FAMILY, QUERY_SIZE, 400)
            } else {
                lead - painter.measure(&text[at..window.start], CODE_FAMILY, QUERY_SIZE, 400)
            }
        })
        .collect();
    FieldView { left, offsets }
}

/// Whether a point in logical window coordinates lands on the regex
/// toggle; the geometry mirrors `draw_bar`.
pub fn toggle_hit(width: f32, px: f32, py: f32) -> bool {
    let x = (width - BAR_WIDTH - MARGIN).max(MARGIN);
    let toggle_x = x + BAR_WIDTH - 10.0 - TOGGLE_W;
    let toggle_y = MARGIN + (BAR_HEIGHT - TOGGLE_H) / 2.0;
    (toggle_x..toggle_x + TOGGLE_W).contains(&px) && (toggle_y..toggle_y + TOGGLE_H).contains(&py)
}

/// The slice of a long query that gets drawn, as a byte range that always
/// contains the caret, and whether characters were cut from the left. The
/// tail is preferred, since that is where typing happens; a caret left of
/// the tail window anchors the window on itself instead.
fn window_fit(
    painter: &mut Painter,
    query: &str,
    caret: usize,
    avail: f32,
) -> (Range<usize>, bool) {
    let fits = |painter: &mut Painter, text: &str| {
        painter.measure(text, CODE_FAMILY, QUERY_SIZE, 400) <= avail
    };
    if fits(painter, query) {
        return (0..query.len(), false);
    }
    let mut start = query.len();
    for (index, _) in query.char_indices().skip(1) {
        if fits(painter, &format!("\u{2026}{}", &query[index..])) {
            start = index;
            break;
        }
    }
    if start <= caret {
        return (start..query.len(), true);
    }
    let mut end = caret;
    for (offset, c) in query[caret..].char_indices() {
        let candidate = caret + offset + c.len_utf8();
        if !fits(painter, &format!("\u{2026}{}", &query[caret..candidate])) {
            break;
        }
        end = candidate;
    }
    (caret..end, caret > 0)
}

/// A query with any capital letter matches exactly; an all-lowercase
/// query matches case-insensitively.
pub fn smart_case_sensitive(query: &str) -> bool {
    query.chars().any(|c| c.is_uppercase())
}

/// Lowercase fold for matching. A multi-character expansion keeps only
/// its first character so haystack and needle stay aligned one to one.
fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// One run of addressable text: `text[range]` maps linearly into the
/// span's text from its start.
struct Seg {
    range: Range<usize>,
    block: usize,
    span: usize,
}

/// The document's display text flattened into one string, walked from
/// the model through the same pieces the copies use, beside the table
/// mapping its addressable bytes back to model positions. Separators,
/// joiners and labels contribute their characters but no segment, so a
/// range covering them alone converts to no selection.
struct Haystack {
    text: String,
    segs: Vec<Seg>,
}

impl Haystack {
    fn build(doc: &Document) -> Haystack {
        let mut hay = Haystack {
            text: String::new(),
            segs: Vec::new(),
        };
        for index in 0..doc.blocks.len() {
            let mut opened = false;
            for piece in block_pieces(doc, index) {
                match piece {
                    Piece::Sep(text) => hay.push_inert(&mut opened, text),
                    Piece::Label(text) => hay.push_inert(&mut opened, &text),
                    Piece::Addr { span, text } => {
                        // An empty piece (an empty code line, an empty
                        // cell) keeps a zero-length segment, so the
                        // joiners around it stand between two segments
                        // and a match made of them maps exactly.
                        hay.open(&mut opened);
                        hay.segs.push(Seg {
                            range: hay.text.len()..hay.text.len() + text.len(),
                            block: index,
                            span,
                        });
                        hay.text.push_str(&text);
                    }
                }
            }
        }
        hay
    }

    /// Plants the block boundary ahead of a block's first character:
    /// newline, separator, newline, so `^` and `$` treat every block as
    /// its own line while the separator keeps any match from crossing.
    fn open(&mut self, opened: &mut bool) {
        if !*opened {
            if !self.text.is_empty() {
                self.text.push('\n');
                self.text.push(BLOCK_SEP);
                self.text.push('\n');
            }
            *opened = true;
        }
    }

    fn push_inert(&mut self, opened: &mut bool, text: &str) {
        if text.is_empty() {
            return;
        }
        self.open(opened);
        self.text.push_str(text);
    }

    /// The selection a byte range of `text` converts to, clipped to the
    /// addressable characters inside it. An empty range, a range covering
    /// the block separator, or one holding neither an addressable byte
    /// nor a line joiner converts to nothing.
    ///
    /// A line joiner is addressed through its neighbors: a range starting
    /// on the `\n` right after a segment starts at that segment's end, and
    /// one ending right after a `\n` that a segment follows ends at that
    /// segment's start. So a match made of line breaks alone selects from
    /// the end of one piece to the start of the next, and never across
    /// blocks, since the `\n` beside the separator has a segment on one
    /// side only and the two ends meet.
    fn selection(&self, range: Range<usize>) -> Option<Selection> {
        if range.is_empty() || self.text[range.clone()].contains(BLOCK_SEP) {
            return None;
        }
        let bytes = self.text.as_bytes();
        let at = |seg: &Seg, byte: usize| ModelPos {
            block: seg.block,
            span: seg.span,
            byte,
        };
        let first = self
            .segs
            .partition_point(|seg| seg.range.end <= range.start);
        let start = match first.checked_sub(1).map(|i| &self.segs[i]) {
            Some(before) if bytes[range.start] == b'\n' && before.range.end == range.start => {
                at(before, before.range.len())
            }
            _ => {
                let head = self.segs.get(first)?;
                at(head, range.start.saturating_sub(head.range.start))
            }
        };
        let last = self.segs.partition_point(|seg| seg.range.start < range.end);
        let end = match self.segs.get(last) {
            Some(after) if bytes[range.end - 1] == b'\n' && after.range.start == range.end => {
                at(after, 0)
            }
            _ => {
                let tail = &self.segs[last.checked_sub(1)?];
                at(tail, range.end.min(tail.range.end) - tail.range.start)
            }
        };
        (start < end).then_some(Selection { start, end })
    }
}

/// Every match in document order, found in the model's display text, so
/// matching needs no layout. Display joiners (tabs between table cells,
/// newlines between rows and code lines) keep a phrase from matching
/// across them, and blocks never join. Empty query, no matches.
pub fn matches(doc: &Document, query: &str) -> Vec<Selection> {
    if query.is_empty() {
        return Vec::new();
    }
    let hay = Haystack::build(doc);
    let found = if smart_case_sensitive(query) {
        find_exact(&hay.text, query)
    } else {
        find_folded(&hay.text, query)
    };
    found
        .into_iter()
        .filter_map(|range| hay.selection(range))
        .collect()
}

/// Every regex match in document order, or `None` when the pattern does
/// not compile or exceeds the engine's backtracking limit, which the bar
/// reports as an invalid pattern. `^` and `$` match at line starts and
/// ends. Case follows smart case read from the pattern's literal
/// characters. A match covering the block separator converts to no
/// selection, so matches still never cross blocks.
pub fn regex_matches(doc: &Document, pattern: &str) -> Option<Vec<Selection>> {
    if pattern.is_empty() {
        return Some(Vec::new());
    }
    let regex = compile(pattern)?;
    let hay = Haystack::build(doc);
    let mut out = Vec::new();
    let mut pos = 0;
    while pos <= hay.text.len() {
        let Some(found) = regex.find_from_pos(&hay.text, pos).ok()? else {
            break;
        };
        match hay.selection(found.start()..found.end()) {
            Some(selection) => {
                out.push(selection);
                pos = found.end();
            }
            // A zero-width or boundary-covering candidate: resume just
            // inside it, so a greedy match crossing a block cannot hide
            // the real matches it covered.
            None => pos = next_boundary(&hay.text, found.start()),
        }
    }
    Some(out)
}

/// The next character boundary past `at`, or past the end so a caller's
/// scan terminates.
fn next_boundary(text: &str, at: usize) -> usize {
    text[at..]
        .chars()
        .next()
        .map_or(text.len() + 1, |c| at + c.len_utf8())
}

/// Every regex match beside its expanded replacement: `$1`, `$0` and
/// `${name}` read the match's own captures and `$$` is a literal dollar.
/// The selections equal `regex_matches` for the same pattern, index for
/// index, and `None` reports the same failures.
pub fn regex_replacements(
    doc: &Document,
    pattern: &str,
    template: &str,
) -> Option<Vec<(Selection, String)>> {
    if pattern.is_empty() {
        return Some(Vec::new());
    }
    let regex = compile(pattern)?;
    let hay = Haystack::build(doc);
    let template = unescape_template(template);
    let expander = fancy_regex::Expander::default();
    let mut out = Vec::new();
    // The walk mirrors `regex_matches` exactly, discard recovery
    // included, which is what keeps the pairs aligned with the matches.
    let mut pos = 0;
    while pos <= hay.text.len() {
        let Some(captures) = regex.captures_from_pos(&hay.text, pos).ok()? else {
            break;
        };
        let found = captures.get(0).expect("a match always captures group 0");
        match hay.selection(found.start()..found.end()) {
            Some(selection) => {
                out.push((selection, expander.expansion(&template, &captures)));
                pos = found.end();
            }
            None => pos = next_boundary(&hay.text, found.start()),
        }
    }
    Some(out)
}

/// The replacement with its escapes read, the way VS Code's replace box
/// reads them in regex mode: `\n` a line break, `\t` a tab, `\\` one
/// backslash. Any other backslash sequence stays as typed, and a
/// backslash ending the template keeps itself. The group syntax is
/// dollar-based, so this pass and the expander never meet.
fn unescape_template(template: &str) -> Cow<'_, str> {
    if !template.contains('\\') {
        return Cow::Borrowed(template);
    }
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    Cow::Owned(out)
}

/// The pattern with the standing flags: anchors match per line, and
/// smart case adds insensitivity.
fn compile(pattern: &str) -> Option<fancy_regex::Regex> {
    let flags = if regex_case_sensitive(pattern) {
        "(?m)"
    } else {
        "(?mi)"
    };
    fancy_regex::Regex::new(&format!("{flags}{pattern}")).ok()
}

/// Smart case for a pattern: a capital letter typed literally makes it
/// case-sensitive; the letter of an escape like `\W` does not count.
fn regex_case_sensitive(pattern: &str) -> bool {
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            chars.next();
        } else if c.is_uppercase() {
            return true;
        }
    }
    false
}

/// Byte ranges of every occurrence in order, non-overlapping: the scan
/// resumes at each match's end. An empty needle finds nothing, which
/// also keeps the scan from standing still.
fn find_exact(text: &str, query: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    if query.is_empty() {
        return out;
    }
    let mut from = 0;
    while let Some(at) = text[from..].find(query) {
        let start = from + at;
        out.push(start..start + query.len());
        from = start + query.len();
    }
    out
}

/// Case-insensitive occurrences: haystack and needle fold to the same
/// alphabet, and each match maps back through the per-character offset
/// table, since folding can change a character's byte length.
fn find_folded(text: &str, query: &str) -> Vec<Range<usize>> {
    let needle: String = query.chars().map(fold).collect();
    let mut folded = String::with_capacity(text.len());
    let mut map: Vec<(usize, usize)> = Vec::new();
    for (at, c) in text.char_indices() {
        map.push((folded.len(), at));
        folded.push(fold(c));
    }
    map.push((folded.len(), text.len()));
    let seat = |at: usize| map[map.partition_point(|&(f, _)| f < at)].1;
    find_exact(&folded, &needle)
        .into_iter()
        .map(|range| seat(range.start)..seat(range.end))
        .collect()
}

/// The neighboring match index with wraparound in either direction.
pub fn step(current: usize, len: usize, forward: bool) -> usize {
    if len == 0 {
        return 0;
    }
    if forward {
        (current + 1) % len
    } else {
        (current + len - 1) % len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::markdown;
    use crate::ui::selection::ModelPos;

    #[test]
    fn matches_reach_inside_closed_details() {
        let doc = markdown::parse(
            "<details>\n<summary>S</summary>\n\nthe needle hides here\n\n</details>",
        );
        assert!(!doc.block_visible(1), "the paragraph is folded away");
        assert_eq!(matches(&doc, "needle").len(), 1);
    }

    #[test]
    fn lowercase_query_matches_any_case() {
        assert!(!smart_case_sensitive("panel"));
        assert!(!smart_case_sensitive("3/17"));
        let doc = markdown::parse("Panel PANEL panel");
        assert_eq!(matches(&doc, "panel").len(), 3);
    }

    #[test]
    fn capital_query_matches_exactly() {
        assert!(smart_case_sensitive("Panel"));
        let doc = markdown::parse("Panel PANEL panel");
        let found = matches(&doc, "Panel");
        assert_eq!(
            found,
            vec![Selection {
                start: ModelPos {
                    block: 0,
                    span: 0,
                    byte: 0
                },
                end: ModelPos {
                    block: 0,
                    span: 0,
                    byte: 5
                },
            }]
        );
    }

    #[test]
    fn match_crosses_a_style_boundary() {
        let doc = markdown::parse("**pan**el rest");
        let found = matches(&doc, "panel");
        assert_eq!(
            found,
            vec![Selection {
                start: ModelPos {
                    block: 0,
                    span: 0,
                    byte: 0
                },
                end: ModelPos {
                    block: 0,
                    span: 1,
                    byte: 2
                },
            }]
        );
    }

    #[test]
    fn no_match_across_blocks() {
        let doc = markdown::parse("one\n\ntwo");
        assert!(matches(&doc, "onetwo").is_empty());
        assert!(matches(&doc, "netw").is_empty());
    }

    #[test]
    fn no_match_across_table_cells() {
        let doc = markdown::parse("| ab | cd |\n|---|---|\n| ef | gh |");
        assert!(matches(&doc, "bc").is_empty());
        assert!(matches(&doc, "fg").is_empty());
        assert_eq!(matches(&doc, "ef").len(), 1);
    }

    #[test]
    fn no_match_across_a_hard_break() {
        let doc = markdown::parse("one two\\\nthree");
        assert!(matches(&doc, "two three").is_empty());
        assert_eq!(matches(&doc, "three").len(), 1);
    }

    #[test]
    fn markers_stay_out_of_the_text() {
        let doc = markdown::parse("- item one");
        let found = matches(&doc, "item");
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].start,
            ModelPos {
                block: 0,
                span: 0,
                byte: 0
            },
            "the bullet is not part of the text"
        );
    }

    #[test]
    fn matches_span_code_lines_but_not_line_breaks() {
        let doc = markdown::parse("```rust\nlet alpha = 1;\nlet beta = 2;\n```");
        assert_eq!(matches(&doc, "alpha").len(), 1);
        assert!(matches(&doc, "1;let").is_empty(), "lines never join");
    }

    #[test]
    fn successive_matches_do_not_overlap() {
        let doc = markdown::parse("aaaa");
        assert_eq!(matches(&doc, "aa").len(), 2);
    }

    #[test]
    fn empty_query_finds_nothing() {
        let doc = markdown::parse("anything");
        assert!(matches(&doc, "").is_empty());
    }

    #[test]
    fn step_wraps_both_ways() {
        assert_eq!(step(0, 3, true), 1);
        assert_eq!(step(2, 3, true), 0);
        assert_eq!(step(0, 3, false), 2);
        assert_eq!(step(0, 1, true), 0);
    }

    #[test]
    fn regex_finds_a_pattern() {
        let doc = markdown::parse("alpha beta");
        let found = regex_matches(&doc, r"b\w+").expect("valid pattern");
        assert_eq!(
            found,
            vec![Selection {
                start: ModelPos {
                    block: 0,
                    span: 0,
                    byte: 6
                },
                end: ModelPos {
                    block: 0,
                    span: 0,
                    byte: 10
                },
            }]
        );
    }

    #[test]
    fn regex_smart_case_reads_literals_only() {
        let doc = markdown::parse("Panel PANEL panel");
        assert_eq!(regex_matches(&doc, "panel").expect("valid").len(), 3);
        assert_eq!(regex_matches(&doc, "Panel").expect("valid").len(), 1);
        let doc = markdown::parse("A B");
        assert_eq!(regex_matches(&doc, r"a\Wb").expect("valid").len(), 1);
    }

    #[test]
    fn regex_zero_width_terminates_and_finds_nothing() {
        let doc = markdown::parse("abc");
        assert!(regex_matches(&doc, "x*").expect("valid").is_empty());
        assert!(regex_matches(&doc, r"\b").expect("valid").is_empty());
    }

    #[test]
    fn regex_never_crosses_blocks() {
        let doc = markdown::parse("one\n\ntwo");
        assert!(regex_matches(&doc, "one.two").expect("valid").is_empty());
    }

    #[test]
    fn regex_crosses_a_joiner_only_when_named() {
        let doc = markdown::parse("```rust\nlet alpha = 1;\nlet beta = 2;\n```");
        assert_eq!(regex_matches(&doc, "1;\nlet").expect("valid").len(), 1);
        assert!(regex_matches(&doc, "1;.let").expect("valid").is_empty());
    }

    #[test]
    fn regex_anchors_match_line_starts() {
        let doc = markdown::parse("```rust\nlet alpha = 1;\nlet beta = 2;\n```");
        assert_eq!(regex_matches(&doc, "^let").expect("valid").len(), 2);
    }

    #[test]
    fn regex_replacements_expand_captures() {
        let doc = markdown::parse("ab cd");
        let pairs = regex_replacements(&doc, r"(\w)(\w)", "$2$1").expect("valid");
        let texts: Vec<&str> = pairs.iter().map(|(_, text)| text.as_str()).collect();
        assert_eq!(texts, ["ba", "dc"]);
        let sels: Vec<Selection> = pairs.iter().map(|(sel, _)| *sel).collect();
        assert_eq!(
            sels,
            regex_matches(&doc, r"(\w)(\w)").expect("valid"),
            "the pairs mirror the match list index for index"
        );
    }

    #[test]
    fn regex_replacement_dollar_forms() {
        let doc = markdown::parse("cat");
        let pairs = regex_replacements(&doc, "c(a)t", "[$0|$1|$$]").expect("valid");
        assert_eq!(pairs[0].1, "[cat|a|$]");
    }

    #[test]
    fn regex_anchors_match_block_edges() {
        let doc = markdown::parse("off\n\nbeta f\n\ngamma");
        let found = regex_matches(&doc, "^.*f.*$").expect("valid");
        assert_eq!(found.len(), 2, "every block holding an f is a line");
        assert_eq!(found[0].start.block, 0);
        assert_eq!(found[1].start.block, 1);
    }

    #[test]
    fn a_discarded_greedy_match_hides_nothing_behind_it() {
        // The greedy candidate from block one spans the separator and is
        // discarded; the scan must resume inside it, not after it.
        let doc = markdown::parse("bar\n\nfoo");
        assert_eq!(regex_matches(&doc, "[^q]*foo").expect("valid").len(), 1);
        assert_eq!(regex_matches(&doc, "(?s).*foo").expect("valid").len(), 1);
        let pairs = regex_replacements(&doc, "[^q]*foo", "$0").expect("valid");
        assert_eq!(
            pairs.len(),
            1,
            "the pairs walk with the same recovery as the matches"
        );
    }

    #[test]
    fn find_exact_refuses_an_empty_needle() {
        assert!(find_exact("abc", "").is_empty());
    }

    #[test]
    fn regex_invalid_pattern_reports_as_invalid() {
        let doc = markdown::parse("anything");
        assert!(regex_matches(&doc, "foo(").is_none());
    }

    #[test]
    fn regex_backtrack_limit_reports_as_invalid() {
        let doc = markdown::parse("x".repeat(40));
        assert!(regex_matches(&doc, r"(?:(x+)\1)+y").is_none());
    }

    // ---- Line breaks as matches: the joiners between pieces ----

    fn code_doc(text: &str) -> Document {
        crate::doc::load::code_document(None, text)
    }

    fn sel(a: (usize, usize, usize), b: (usize, usize, usize)) -> Selection {
        let pos = |(block, span, byte)| ModelPos { block, span, byte };
        Selection {
            start: pos(a),
            end: pos(b),
        }
    }

    /// The source after every pair is spliced, the way replace all does.
    fn replaced_all(doc: &Document, pattern: &str, template: &str) -> String {
        let pairs = regex_replacements(doc, pattern, template).expect("valid");
        let edits: Vec<(Range<usize>, String)> = pairs
            .into_iter()
            .filter_map(|(sel, text)| {
                crate::edit::caret::selection_range(doc, &sel).map(|range| (range, text))
            })
            .collect();
        let (region, text) = crate::edit::splice::combine(&doc.source, &edits).expect("edits");
        let mut out = doc.source.to_string();
        out.replace_range(region, &text);
        out
    }

    #[test]
    fn a_line_break_matches_between_code_lines() {
        let doc = code_doc("one\ntwo\nthree");
        assert_eq!(
            regex_matches(&doc, "\\n").expect("valid"),
            vec![sel((0, 0, 3), (0, 1, 0)), sel((0, 1, 3), (0, 2, 0))]
        );
    }

    #[test]
    fn line_breaks_around_an_empty_line_match_exactly() {
        let doc = code_doc("one\n\ntwo");
        assert_eq!(
            regex_matches(&doc, "\\n\\n").expect("valid"),
            vec![sel((0, 0, 3), (0, 2, 0))]
        );
        assert_eq!(
            regex_matches(&doc, "\\n").expect("valid"),
            vec![sel((0, 0, 3), (0, 1, 0)), sel((0, 1, 0), (0, 2, 0))]
        );
    }

    #[test]
    fn a_line_break_never_joins_blocks() {
        let doc = markdown::parse("one\n\ntwo");
        assert!(regex_matches(&doc, "\\n").expect("valid").is_empty());
        let doc = markdown::parse("one\n\n```\na\nb\n```\n\ntwo");
        assert_eq!(
            regex_matches(&doc, "\\n").expect("valid"),
            vec![sel((1, 0, 1), (1, 1, 0))],
            "only the break inside the code block"
        );
    }

    #[test]
    fn the_final_line_break_is_not_a_joiner() {
        let doc = code_doc("one\ntwo\n");
        assert_eq!(regex_matches(&doc, "\\n").expect("valid").len(), 1);
    }

    #[test]
    fn line_breaks_join_table_rows_and_tabs_join_nothing() {
        let doc = markdown::parse("| a | b |\n|---|---|\n| c | d |\n| e | f |");
        assert_eq!(regex_matches(&doc, "\\n").expect("valid").len(), 2);
        assert!(regex_matches(&doc, "\\t").expect("valid").is_empty());
        assert!(matches(&doc, "\t").is_empty());
    }

    #[test]
    fn a_hard_break_matches_as_a_line_break() {
        let doc = markdown::parse("one two\\\nthree");
        assert_eq!(regex_matches(&doc, "\\n").expect("valid").len(), 1);
        assert!(matches(&doc, "two three").is_empty());
    }

    #[test]
    fn a_named_line_end_and_a_line_break_agree() {
        let doc = code_doc("go\nno\nyes\nso");
        assert_eq!(regex_matches(&doc, "o$").expect("valid").len(), 3);
        let found = regex_matches(&doc, "o\\n").expect("valid");
        assert_eq!(found.len(), 2, "the last line has no break after it");
        assert_eq!(
            found[0],
            sel((0, 0, 1), (0, 1, 0)),
            "the break itself is inside the match"
        );
    }

    #[test]
    fn a_pasted_newline_finds_the_joiners_too() {
        let doc = code_doc("one\ntwo\nthree");
        assert_eq!(matches(&doc, "\n").len(), 2);
    }

    #[test]
    fn line_break_replacements_mirror_the_matches() {
        let doc = code_doc("one\ntwo\nthree");
        let pairs = regex_replacements(&doc, "\\n", " ").expect("valid");
        let sels: Vec<Selection> = pairs.iter().map(|(s, _)| *s).collect();
        assert_eq!(sels, regex_matches(&doc, "\\n").expect("valid"));
        assert!(pairs.iter().all(|(_, t)| t == " "));
    }

    #[test]
    fn replacing_line_breaks_joins_lines() {
        let doc = code_doc("one\ntwo\nthree\n");
        assert_eq!(
            replaced_all(&doc, "\\n", " "),
            "one two three\n",
            "the terminator stays"
        );
    }

    #[test]
    fn replacing_double_blank_lines_collapses_them() {
        let doc = code_doc("one\n\n\ntwo\n");
        assert_eq!(replaced_all(&doc, "\\n\\n\\n", "\n\n"), "one\n\ntwo\n");
    }

    #[test]
    fn replacing_a_line_break_with_two_doubles_it() {
        let doc = code_doc("one\ntwo\n");
        assert_eq!(replaced_all(&doc, "\\n", "\n\n"), "one\n\ntwo\n");
    }

    #[test]
    fn replacing_a_line_break_keeps_the_other_crlf_endings() {
        // The load normalizes CRLF to LF and remembers where; the ledger
        // puts every untouched ending back on save.
        let doc = code_doc("one\ntwo\nthree\n");
        let first = regex_matches(&doc, "\\n").expect("valid")[0];
        let range = crate::edit::caret::selection_range(&doc, &first).expect("a source range");
        let mut led = crate::edit::splice::Ledger::new(doc.source.clone(), vec![3, 7, 13]);
        led.edit(range, " ");
        assert_eq!(led.emit(), b"one two\r\nthree\r\n");
    }

    // ---- Escapes in the replacement ----

    fn replacement_texts(doc: &Document, pattern: &str, template: &str) -> Vec<String> {
        regex_replacements(doc, pattern, template)
            .expect("valid")
            .into_iter()
            .map(|(_, text)| text)
            .collect()
    }

    #[test]
    fn a_tab_and_a_newline_escape_in_the_replacement() {
        let doc = markdown::parse("a b");
        assert_eq!(replacement_texts(&doc, " ", "\\t"), ["\t"]);
        assert_eq!(replacement_texts(&doc, " ", "\\n"), ["\n"]);
        assert_eq!(replacement_texts(&doc, " ", "x\\ty\\nz"), ["x\ty\nz"]);
    }

    #[test]
    fn a_doubled_backslash_is_one_backslash() {
        let doc = markdown::parse("a b");
        assert_eq!(replacement_texts(&doc, " ", "\\\\"), ["\\"]);
        assert_eq!(
            replacement_texts(&doc, " ", "\\\\t"),
            ["\\t"],
            "the escaped backslash does not join the t"
        );
    }

    #[test]
    fn other_escapes_stay_as_typed() {
        let doc = markdown::parse("a b");
        assert_eq!(replacement_texts(&doc, " ", "\\s"), ["\\s"]);
        assert_eq!(replacement_texts(&doc, " ", "\\d"), ["\\d"]);
        assert_eq!(
            replacement_texts(&doc, " ", "end\\"),
            ["end\\"],
            "a lone backslash at the end keeps itself"
        );
    }

    #[test]
    fn a_group_and_an_escape_combine() {
        let doc = markdown::parse("ab cd");
        assert_eq!(
            replacement_texts(&doc, r"(\w)(\w)", "$2\\t$1"),
            ["b\ta", "d\tc"]
        );
        assert_eq!(
            replacement_texts(&doc, r"(\w)(\w)", "$$\\n"),
            ["$\n", "$\n"]
        );
    }
}
