//! The Files tab's search views: a query field under the captions, the
//! ranked paths or the matching lines under it, and nothing else. No
//! walking, no matching, no reading — those belong to the workspace
//! search worker, whose events the app folds into this state. The
//! tree's selection and scroll are never borrowed: the search keeps
//! its own, so leaving it restores the tree exactly, and the file and
//! content queries each keep their own text across switches.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::paint::painter::Painter;
use crate::style::fonts::{BODY_FAMILY, CODE_FAMILY};
use crate::style::theme::Theme;
use crate::ui::overlay::{dim, soft};
use crate::ui::search::{draw_field, FieldView};
use crate::ui::sidebar::{draw_row_ground, CAPTION_H, PAD, ROW_H};
use crate::ui::textfield::TextField;
use crate::workspace_search::{ContentHit, FileHit};

/// Height of the query row below the captions.
pub const SEARCH_ROW_H: f32 = 34.0;
/// Gap between the query row and the results.
const ROW_GAP: f32 = 6.0;
const TEXT_SIZE: f32 = 14.0;
const LINE_SIZE: f32 = 13.0;
/// The regex toggle's box in the content view, right of the field.
const TOGGLE_W: f32 = 26.0;
const TOGGLE_H: f32 = 22.0;
const TOGGLE_GAP: f32 = 8.0;
/// The line-number column's width in the content view.
const GUTTER_W: f32 = 40.0;

/// Which of the Files tab's views is showing; the tree is one of them,
/// so the tab's state is complete only with this.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FilesView {
    Tree,
    FileSearch,
    ContentSearch,
}

/// Where the search stands, for the status line the results carry.
pub enum SearchStatus {
    /// No question yet; the field's hint counts the index.
    Idle { files: usize },
    /// The root is being walked.
    Indexing,
    /// A query is running.
    Searching,
    /// The answer is complete.
    Done { truncated: bool, skipped: usize },
    /// The pattern did not compile.
    InvalidPattern,
    /// The root could not be searched at all.
    Error(String),
}

/// One visible row of the content view: a file's header, or a line
/// hit at an index into `content`.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ContentRow {
    Header(usize),
    Hit(usize),
}

pub struct SidebarSearchState {
    pub view: FilesView,
    pub file_query: TextField,
    pub content_query: TextField,
    /// Regex matching in the content view, kept across switches like
    /// the queries.
    pub regex: bool,
    /// The selected result, an index into the visible rows.
    pub selected: usize,
    pub scroll: f32,
    pub status: SearchStatus,
    pub files: Vec<FileHit>,
    pub content: Vec<ContentHit>,
    /// The content view's flattened rows, rebuilt when `content` grows.
    pub rows: Vec<ContentRow>,
    /// The content view's files folded to their header row, by path.
    /// Batches keep arriving into the folded groups unseen.
    pub collapsed: HashSet<String>,
    /// Where each query's text was last drawn, for the mouse.
    pub file_view: FieldView,
    pub content_view: FieldView,
}

impl Default for SidebarSearchState {
    fn default() -> SidebarSearchState {
        SidebarSearchState::new()
    }
}

impl SidebarSearchState {
    pub fn new() -> SidebarSearchState {
        SidebarSearchState {
            view: FilesView::Tree,
            file_query: TextField::new(""),
            content_query: TextField::new(""),
            regex: false,
            selected: 0,
            scroll: 0.0,
            status: SearchStatus::Idle { files: 0 },
            files: Vec::new(),
            content: Vec::new(),
            rows: Vec::new(),
            collapsed: HashSet::new(),
            file_view: FieldView::default(),
            content_view: FieldView::default(),
        }
    }

    /// Whether one of the search views is showing.
    pub fn active(&self) -> bool {
        self.view != FilesView::Tree
    }

    /// Enters a search view, keeping whatever the queries hold.
    pub fn open(&mut self, view: FilesView) {
        self.view = view;
        self.selected = 0;
        self.scroll = 0.0;
    }

    /// Returns to the tree; the queries stay for the next entry.
    pub fn close(&mut self) {
        self.view = FilesView::Tree;
        self.files.clear();
        self.content.clear();
        self.rows.clear();
        self.collapsed.clear();
        self.selected = 0;
        self.scroll = 0.0;
    }

    /// The field the active view types into.
    pub fn query_field(&self) -> &TextField {
        match self.view {
            FilesView::ContentSearch => &self.content_query,
            _ => &self.file_query,
        }
    }

    pub fn query_field_mut(&mut self) -> &mut TextField {
        match self.view {
            FilesView::ContentSearch => &mut self.content_query,
            _ => &mut self.file_query,
        }
    }

    /// Where the active view's query text was last drawn.
    pub fn query_view(&self) -> &FieldView {
        match self.view {
            FilesView::ContentSearch => &self.content_view,
            _ => &self.file_view,
        }
    }

    /// Rebuilds the content view's rows after its hits changed. A
    /// folded file keeps its header and loses its lines.
    pub fn refresh_rows(&mut self) {
        self.rows.clear();
        let mut last: Option<&str> = None;
        for (at, hit) in self.content.iter().enumerate() {
            if last != Some(&hit.relative_path) {
                self.rows.push(ContentRow::Header(at));
                last = Some(&hit.relative_path);
            }
            if self.collapsed.contains(&*hit.relative_path) {
                continue;
            }
            self.rows.push(ContentRow::Hit(at));
        }
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
    }

    /// Folds or unfolds one file's group, and seats the selection on
    /// its header either way, so a fold never strands the selection on
    /// a row that vanished.
    pub fn toggle_file(&mut self, path: &str) {
        if !self.collapsed.insert(path.to_string()) {
            self.collapsed.remove(path);
        }
        self.refresh_rows();
        let seat = self.rows.iter().position(|row| match row {
            ContentRow::Header(at) => &*self.content[*at].relative_path == path,
            ContentRow::Hit(_) => false,
        });
        if let Some(at) = seat {
            self.selected = at;
        }
    }

    /// Empties the standing answer for a fresh question: the hits and
    /// rows go, the folds stay for the files that stay. Without this,
    /// a new query's batches append below the old query's lines and
    /// the list reads as never having updated.
    pub fn begin_query(&mut self) {
        self.files.clear();
        self.content.clear();
        self.rows.clear();
        self.selected = 0;
        self.scroll = 0.0;
    }

    /// The results' full height, headers included where there are any.
    pub fn content_h(&self) -> f32 {
        match self.view {
            FilesView::FileSearch => self.files.len() as f32 * ROW_H,
            FilesView::ContentSearch => self.rows.len() as f32 * ROW_H,
            FilesView::Tree => 0.0,
        }
    }

    fn max_scroll(&self, list_h: f32) -> f32 {
        (self.content_h() - list_h).max(0.0)
    }

    /// Moves the selection between results — a file a row in the file
    /// view, a matching line a row with the content view's headers
    /// stepped over — and keeps the selection inside the viewport. A
    /// zero delta only re-seats the scroll, which a resize owes.
    pub fn move_selection(&mut self, delta: i32, list_h: f32) {
        let len = match self.view {
            FilesView::FileSearch => self.files.len(),
            FilesView::ContentSearch => self.rows.len(),
            FilesView::Tree => 0,
        };
        if len == 0 {
            return;
        }
        if delta == 0 {
            self.scroll_to_selection(list_h);
            return;
        }
        let headers = self.view == FilesView::ContentSearch;
        for _ in 0..delta.unsigned_abs() {
            if delta > 0 {
                let mut at = (self.selected + 1).min(len - 1);
                if headers {
                    while at < len && matches!(self.rows.get(at), Some(ContentRow::Header(_))) {
                        at += 1;
                    }
                    at = at.min(len - 1);
                }
                self.selected = at;
            } else {
                let mut at = self.selected.saturating_sub(1);
                if headers {
                    while at > 0 && matches!(self.rows.get(at), Some(ContentRow::Header(_))) {
                        at -= 1;
                    }
                }
                self.selected = at;
            }
        }
        // A clamp at either end can still land on the first header;
        // the nearest line takes it.
        if headers && matches!(self.rows.get(self.selected), Some(ContentRow::Header(_))) {
            self.selected = self
                .rows
                .iter()
                .position(|row| matches!(row, ContentRow::Hit(_)))
                .unwrap_or(self.selected);
        }
        self.scroll_to_selection(list_h);
    }

    /// Moves the selection by a viewport's worth of results.
    pub fn page(&mut self, down: bool, list_h: f32) {
        let page = ((list_h / ROW_H).floor() as usize).saturating_sub(1).max(1);
        self.move_selection(if down { page as i32 } else { -(page as i32) }, list_h);
    }

    fn scroll_to_selection(&mut self, list_h: f32) {
        let top = self.selected as f32 * ROW_H;
        let list_h = list_h.max(ROW_H);
        if top < self.scroll {
            self.scroll = top;
        } else if top + ROW_H > self.scroll + list_h {
            self.scroll = top + ROW_H - list_h;
        }
        self.scroll = self.scroll.clamp(0.0, self.max_scroll(list_h));
    }

    /// The selected file hit, when the file view is showing one.
    pub fn selected_file(&self) -> Option<&FileHit> {
        (self.view == FilesView::FileSearch).then(|| self.files.get(self.selected))?
    }

    /// The selected line hit, when the content view is showing one.
    pub fn selected_line(&self) -> Option<&ContentHit> {
        if self.view != FilesView::ContentSearch {
            return None;
        }
        match self.rows.get(self.selected) {
            Some(ContentRow::Hit(at)) => self.content.get(*at),
            _ => None,
        }
    }

    /// Clamps the scroll after the viewport changed size.
    pub fn clamp_scroll(&mut self, list_h: f32) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll(list_h));
    }
}

/// The top of the results viewport, below the query row.
pub fn results_top() -> f32 {
    PAD + CAPTION_H + SEARCH_ROW_H + ROW_GAP
}

/// The query field's box: x, y, width and height, leaving room for the
/// regex toggle in the content view.
pub fn field_rect(width: f32, content: bool) -> (f32, f32, f32, f32) {
    let toggle = if content { TOGGLE_W + TOGGLE_GAP } else { 0.0 };
    (
        PAD,
        PAD + CAPTION_H + 6.0,
        (width - 2.0 * PAD - toggle).max(40.0),
        SEARCH_ROW_H - 10.0,
    )
}

/// The regex toggle's box in the content view.
pub fn toggle_rect(width: f32) -> (f32, f32, f32, f32) {
    (
        width - PAD - TOGGLE_W,
        PAD + CAPTION_H + (SEARCH_ROW_H - TOGGLE_H) / 2.0,
        TOGGLE_W,
        TOGGLE_H,
    )
}

/// The result row a click at panel `y` lands on, if any.
pub fn row_at(state: &SidebarSearchState, y: f32) -> Option<usize> {
    let index = ((y - results_top() + state.scroll) / ROW_H).floor();
    let len = match state.view {
        FilesView::FileSearch => state.files.len(),
        FilesView::ContentSearch => state.rows.len(),
        FilesView::Tree => 0,
    };
    (index >= 0.0 && (index as usize) < len).then_some(index as usize)
}

/// An indexed path's absolute form: the root joined with the stored
/// `/`-separated relative path, in the platform's own separators.
pub fn absolute(root: &Path, relative: &str) -> PathBuf {
    root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR))
}

/// Draws the search chrome below the captions: the query field, and
/// beside it in the content view the regex toggle.
pub fn draw_field_row(
    painter: &mut Painter,
    theme: &Theme,
    width: f32,
    state: &mut SidebarSearchState,
    owns_keys: bool,
) {
    let content = state.view == FilesView::ContentSearch;
    let (fx, fy, fw, _) = field_rect(width, content);
    let placeholder = if content {
        "Search in files..."
    } else {
        "Find files..."
    };
    let view = draw_field(
        painter,
        theme,
        state.query_field(),
        fx,
        fy + 5.0,
        fw - 6.0,
        placeholder,
        owns_keys,
    );
    match state.view {
        FilesView::ContentSearch => {
            state.content_view = view;
            draw_toggle(painter, theme, width, state.regex);
        }
        _ => state.file_view = view,
    }
}

/// The `.*` toggle, filled with the selection color while regex is on.
fn draw_toggle(painter: &mut Painter, theme: &Theme, width: f32, on: bool) {
    let (tx, ty, tw, th) = toggle_rect(width);
    if on {
        painter.fill(tx, ty, tw, th, 6.0, theme.ui.selection_bg);
    }
    let label_w = painter.measure(".*", CODE_FAMILY, 12.0, 600);
    let color = if on {
        theme.ui.overlay_fg
    } else {
        dim(theme.ui.sidebar_fg)
    };
    painter.text(
        tx + (tw - label_w) / 2.0,
        ty + 2.0,
        ".*",
        CODE_FAMILY,
        12.0,
        600,
        color,
    );
}

/// Draws the results viewport's contents; the caller clipped to it.
pub fn draw_results(
    painter: &mut Painter,
    theme: &Theme,
    width: f32,
    list_h: f32,
    state: &mut SidebarSearchState,
    owns_keys: bool,
) {
    state.clamp_scroll(list_h);
    match state.view {
        FilesView::Tree => {}
        FilesView::FileSearch => draw_file_results(painter, theme, width, state, owns_keys),
        FilesView::ContentSearch => {
            draw_content_results(painter, theme, width, list_h, state, owns_keys)
        }
    }
}

fn draw_file_results(
    painter: &mut Painter,
    theme: &Theme,
    width: f32,
    state: &mut SidebarSearchState,
    owns_keys: bool,
) {
    let ui = &theme.ui;
    let query = state.file_query.text();
    if query.is_empty()
        || (state.files.is_empty()
            && matches!(
                state.status,
                SearchStatus::Idle { .. } | SearchStatus::Indexing
            ))
    {
        draw_status(painter, theme, width, state);
        return;
    }
    if state.files.is_empty() {
        draw_status(painter, theme, width, state);
        return;
    }
    let top = results_top();
    let first = (state.scroll / ROW_H).floor() as usize;
    let offset = -(state.scroll - first as f32 * ROW_H);
    let mut slot = 0usize;
    loop {
        let index = first + slot;
        let ry = top + offset + slot as f32 * ROW_H;
        if index >= state.files.len() || ry > painter.height() - PAD {
            break;
        }
        slot += 1;
        let attended = owns_keys && index == state.selected;
        draw_row_ground(
            painter,
            width,
            ry,
            false,
            attended,
            ui.sidebar_fg,
            ui.sidebar_dir,
        );
        draw_hit_path(painter, theme, width, ry, &state.files[index], attended);
    }
    if matches!(
        state.status,
        SearchStatus::Done {
            truncated: true,
            ..
        }
    ) {
        let note = format!("{}+ matches", state.files.len());
        painter.text(
            PAD,
            top + offset + state.files.len() as f32 * ROW_H + 8.0,
            &note,
            BODY_FAMILY,
            TEXT_SIZE,
            400,
            dim(ui.sidebar_fg),
        );
    }
}

/// One file result: the whole path in the panel's text color with the
/// matched characters accented, truncated to fit.
#[allow(clippy::too_many_arguments)]
fn draw_hit_path(
    painter: &mut Painter,
    theme: &Theme,
    width: f32,
    ry: f32,
    hit: &FileHit,
    attended: bool,
) {
    let ui = &theme.ui;
    let fg = if attended {
        ui.sidebar_dir
    } else {
        ui.sidebar_fg
    };
    let accent = ui.sidebar_dir;
    let avail = width - 2.0 * PAD;
    let full = &*hit.relative_path;
    let shown = crate::ui::sidebar::fit(full, avail, |text| {
        painter.measure(text, BODY_FAMILY, TEXT_SIZE, 400)
    });
    let cut = shown.len();
    let name_at = (hit.basename_start as usize).min(cut);
    // Segments alternate: a run of plain text, then each matched
    // character (or consecutive run of them) accented. A plain run
    // that crosses from the directory into the file's own name splits
    // there, so the directory reads dimmed the way the tree reads it.
    // Offsets beyond what the fit kept are gone with the characters
    // they named.
    let mut x = PAD;
    let mut at = 0usize;
    let mut matched = hit
        .matched
        .iter()
        .copied()
        .filter(|m| (*m as usize) < cut)
        .peekable();
    while at < cut {
        let (end, this_accent) = match matched.peek() {
            Some(m) if *m as usize == at => {
                matched.next();
                let mut end = at + 1;
                while matched.peek() == Some(&(end as u32)) {
                    matched.next();
                    end += 1;
                }
                (end, true)
            }
            Some(m) => ((*m as usize).min(cut), false),
            None => (cut, false),
        };
        let mut from = at;
        while from < end {
            let stop = if this_accent {
                end
            } else if from < name_at {
                name_at.min(end)
            } else {
                end
            };
            let segment = &shown[from..stop];
            if !segment.is_empty() {
                let color = if this_accent {
                    accent
                } else if from < name_at {
                    dim(fg)
                } else {
                    fg
                };
                painter.text(x, ry + 6.0, segment, BODY_FAMILY, TEXT_SIZE, 400, color);
                x += painter.measure(segment, BODY_FAMILY, TEXT_SIZE, 400);
            }
            from = stop;
        }
        at = end;
    }
}

fn draw_content_results(
    painter: &mut Painter,
    theme: &Theme,
    width: f32,
    list_h: f32,
    state: &mut SidebarSearchState,
    owns_keys: bool,
) {
    let ui = &theme.ui;
    if state.content_query.text().is_empty() || state.rows.is_empty() {
        draw_status(painter, theme, width, state);
        return;
    }
    let top = results_top();
    let first = (state.scroll / ROW_H).floor() as usize;
    let offset = -(state.scroll - first as f32 * ROW_H);
    let mut slot = 0usize;
    loop {
        let index = first + slot;
        let ry = top + offset + slot as f32 * ROW_H;
        if index >= state.rows.len() || ry > painter.height() - PAD {
            break;
        }
        slot += 1;
        match state.rows[index] {
            ContentRow::Header(at) => {
                // The fold's own row: a triangle for its state, the
                // path beside it, the click on either folding the file.
                let path = state.content[at].relative_path.clone();
                let folded = state.collapsed.contains(&*path);
                crate::ui::sidebar::draw_triangle(
                    painter,
                    PAD + 5.0,
                    ry + ROW_H / 2.0,
                    !folded,
                    soft(ui.sidebar_fg),
                );
                let avail = width - PAD - 18.0;
                let shown = crate::ui::sidebar::fit(&path, avail, |text| {
                    painter.measure(text, BODY_FAMILY, TEXT_SIZE, 700)
                });
                painter.text(
                    PAD + 16.0,
                    ry + 6.0,
                    &shown,
                    BODY_FAMILY,
                    TEXT_SIZE,
                    700,
                    soft(ui.sidebar_fg),
                );
            }
            ContentRow::Hit(at) => {
                let attended = owns_keys && index == state.selected;
                draw_row_ground(
                    painter,
                    width,
                    ry,
                    false,
                    attended,
                    ui.sidebar_fg,
                    ui.sidebar_dir,
                );
                draw_hit_line(painter, theme, width, ry, &state.content[at], attended);
            }
        }
    }
    let _ = list_h;
    if matches!(
        state.status,
        SearchStatus::Done {
            truncated: true,
            ..
        }
    ) {
        let note = format!("{}+ matches", state.content.len());
        painter.text(
            PAD,
            top + offset + state.rows.len() as f32 * ROW_H + 8.0,
            &note,
            BODY_FAMILY,
            TEXT_SIZE,
            400,
            dim(ui.sidebar_fg),
        );
    }
}

/// One matching line: the line number in the gutter, the line's text
/// with the matched ranges accented.
fn draw_hit_line(
    painter: &mut Painter,
    theme: &Theme,
    width: f32,
    ry: f32,
    hit: &ContentHit,
    attended: bool,
) {
    let ui = &theme.ui;
    let fg = if attended {
        ui.sidebar_dir
    } else {
        ui.sidebar_fg
    };
    let accent = ui.sidebar_dir;
    let number = hit.line_number.to_string();
    let number = crate::ui::sidebar::fit(&number, GUTTER_W - 8.0, |text| {
        painter.measure(text, CODE_FAMILY, LINE_SIZE, 400)
    });
    painter.text(
        PAD + 2.0,
        ry + 7.0,
        &number,
        CODE_FAMILY,
        LINE_SIZE,
        400,
        dim(ui.sidebar_fg),
    );
    let text_x = PAD + GUTTER_W;
    let avail = width - text_x - PAD;
    let line = hit.line_text.trim_end_matches(['\r', '\n']);
    let shown = crate::ui::sidebar::fit(line, avail, |text| {
        painter.measure(text, CODE_FAMILY, LINE_SIZE, 400)
    });
    let cut = shown.len();
    let mut x = text_x;
    let mut at = 0usize;
    let mut ranges = hit.ranges.iter().filter(|r| r.start < cut);
    while at < cut {
        let (end, this_accent) = match ranges.clone().next() {
            Some(range) if range.start == at => {
                ranges.next();
                (range.end.min(cut).max(at + 1), true)
            }
            Some(range) => (range.start.min(cut), false),
            None => (cut, false),
        };
        let segment = &shown[at..end];
        if !segment.is_empty() {
            let color = if this_accent { accent } else { fg };
            painter.text(x, ry + 7.0, segment, CODE_FAMILY, LINE_SIZE, 400, color);
            x += painter.measure(segment, CODE_FAMILY, LINE_SIZE, 400);
        }
        at = end;
    }
}

/// The one-line word the results area shows while there is nothing to
/// list: the state of the index, of the running query, or of the
/// pattern that would not compile.
fn draw_status(painter: &mut Painter, theme: &Theme, width: f32, state: &SidebarSearchState) {
    let ui = &theme.ui;
    let fg = dim(ui.sidebar_fg);
    let text = match &state.status {
        SearchStatus::Idle { files } => {
            if state.query_field().is_empty() {
                format!("{files} files indexed")
            } else {
                "No matches".to_string()
            }
        }
        SearchStatus::Indexing => "Indexing...".to_string(),
        SearchStatus::Searching => "Searching...".to_string(),
        SearchStatus::Done { skipped, .. } => {
            if *skipped > 0 {
                format!("{} matches \u{b7} {skipped} files skipped", counted(state))
            } else {
                format!("{} matches", counted(state))
            }
        }
        SearchStatus::InvalidPattern => "Invalid pattern".to_string(),
        SearchStatus::Error(message) => message.clone(),
    };
    let color = match &state.status {
        SearchStatus::InvalidPattern | SearchStatus::Error(_) => theme.alerts.caution,
        _ => fg,
    };
    let avail = width - 2.0 * PAD;
    let shown = crate::ui::sidebar::fit(&text, avail, |t| {
        painter.measure(t, BODY_FAMILY, TEXT_SIZE, 400)
    });
    painter.text(
        PAD,
        results_top() + 8.0,
        &shown,
        BODY_FAMILY,
        TEXT_SIZE,
        400,
        color,
    );
}

/// How many results the active view is holding.
fn counted(state: &SidebarSearchState) -> usize {
    match state.view {
        FilesView::FileSearch => state.files.len(),
        FilesView::ContentSearch => state.content.len(),
        FilesView::Tree => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn hit(path: &str) -> FileHit {
        FileHit {
            relative_path: Arc::from(path),
            basename_start: path.rfind('/').map_or(0, |at| at + 1) as u32,
            score: 0,
            matched: Vec::new(),
        }
    }

    fn line(path: &str, number: u64, text: &str) -> ContentHit {
        ContentHit {
            relative_path: Arc::from(path),
            line_number: number,
            line_text: Arc::from(text),
            ranges: Vec::new(),
        }
    }

    #[test]
    fn a_new_state_sits_in_the_tree() {
        let state = SidebarSearchState::new();
        assert_eq!(state.view, FilesView::Tree);
        assert!(!state.active());
    }

    #[test]
    fn opening_and_closing_keeps_the_queries() {
        let mut state = SidebarSearchState::new();
        state.open(FilesView::FileSearch);
        state.file_query.insert("user");
        state.open(FilesView::ContentSearch);
        state.content_query.insert("struct");
        assert_eq!(state.content_query.text(), "struct");
        state.close();
        assert_eq!(state.view, FilesView::Tree);
        state.open(FilesView::FileSearch);
        assert_eq!(state.file_query.text(), "user", "the file query survived");
        state.open(FilesView::ContentSearch);
        assert_eq!(state.content_query.text(), "struct");
    }

    #[test]
    fn the_field_routes_by_view() {
        let mut state = SidebarSearchState::new();
        state.open(FilesView::ContentSearch);
        state.query_field_mut().insert("abc");
        assert_eq!(state.content_query.text(), "abc");
        assert_eq!(state.file_query.text(), "");
        state.open(FilesView::FileSearch);
        state.query_field_mut().insert("xy");
        assert_eq!(state.file_query.text(), "xy");
    }

    #[test]
    fn content_rows_group_by_file_with_headers() {
        let mut state = SidebarSearchState::new();
        state.open(FilesView::ContentSearch);
        state.content = vec![
            line("a.rs", 1, "x"),
            line("a.rs", 2, "y"),
            line("b.rs", 7, "z"),
        ];
        state.refresh_rows();
        assert_eq!(
            state.rows,
            vec![
                ContentRow::Header(0),
                ContentRow::Hit(0),
                ContentRow::Hit(1),
                ContentRow::Header(2),
                ContentRow::Hit(2),
            ]
        );
        assert_eq!(state.content_h(), 5.0 * ROW_H);
    }

    #[test]
    fn selection_moves_between_lines_and_skips_headers() {
        let mut state = SidebarSearchState::new();
        state.open(FilesView::ContentSearch);
        state.content = vec![
            line("a.rs", 1, "x"),
            line("a.rs", 2, "y"),
            line("b.rs", 7, "z"),
        ];
        state.refresh_rows();
        // rows: header(0) hit(0) hit(1) header(2) hit(2)
        state.move_selection(1, 300.0);
        assert_eq!(state.selected, 1, "the first hit after the header");
        state.move_selection(1, 300.0);
        assert_eq!(state.selected, 2);
        state.move_selection(1, 300.0);
        assert_eq!(state.selected, 4, "b.rs's header is skipped");
        state.move_selection(-1, 300.0);
        assert_eq!(state.selected, 2, "and back over it");
        state.move_selection(-20, 300.0);
        assert_eq!(state.selected, 1, "clamped at the first hit");
        state.move_selection(20, 300.0);
        assert_eq!(state.selected, 4, "clamped at the last");
    }

    #[test]
    fn file_selection_moves_and_clamps() {
        let mut state = SidebarSearchState::new();
        state.open(FilesView::FileSearch);
        state.files = vec![hit("a.rs"), hit("b.rs"), hit("c.rs")];
        state.move_selection(1, 300.0);
        assert_eq!(state.selected, 1);
        state.move_selection(-5, 300.0);
        assert_eq!(state.selected, 0);
        state.move_selection(5, 300.0);
        assert_eq!(state.selected, 2);
    }

    #[test]
    fn the_selected_line_reads_through_the_rows() {
        let mut state = SidebarSearchState::new();
        state.open(FilesView::ContentSearch);
        state.content = vec![line("a.rs", 4, " UserService "), line("b.rs", 9, "x")];
        state.refresh_rows();
        state.selected = 1;
        let hit = state.selected_line().unwrap();
        assert_eq!(&*hit.relative_path, "a.rs");
        assert_eq!(hit.line_number, 4);
        state.selected = 0;
        assert!(state.selected_line().is_none(), "the header is not a line");
        state.selected = 3;
        assert_eq!(state.selected_line().unwrap().line_number, 9);
    }

    #[test]
    fn an_absolute_path_joins_the_root_in_native_separator() {
        let root = Path::new(if cfg!(windows) { r"C:\work" } else { "/work" });
        let joined = absolute(root, "src/ui/mod.rs");
        assert!(joined.starts_with(root));
        assert!(joined.ends_with(Path::new("src").join("ui").join("mod.rs")));
    }

    #[test]
    fn a_folded_file_keeps_its_header_and_loses_its_lines() {
        let mut state = SidebarSearchState::new();
        state.open(FilesView::ContentSearch);
        state.content = vec![
            line("a.rs", 1, "x"),
            line("a.rs", 2, "y"),
            line("b.rs", 3, "z"),
        ];
        state.refresh_rows();
        state.toggle_file("a.rs");
        assert_eq!(
            state.rows,
            vec![
                ContentRow::Header(0),
                ContentRow::Header(2),
                ContentRow::Hit(2)
            ],
            "a.rs folds to its header alone"
        );
        assert_eq!(state.selected, 0, "the fold seats the selection");
        // A batch arriving while folded stays hidden until the unfold;
        // the unfolded files take their new lines in.
        state.content.push(line("b.rs", 4, "w"));
        state.refresh_rows();
        assert_eq!(
            state.rows,
            vec![
                ContentRow::Header(0),
                ContentRow::Header(2),
                ContentRow::Hit(2),
                ContentRow::Hit(3),
            ],
            "a.rs stays folded, b.rs grows"
        );
        state.toggle_file("a.rs");
        assert_eq!(
            state.rows,
            vec![
                ContentRow::Header(0),
                ContentRow::Hit(0),
                ContentRow::Hit(1),
                ContentRow::Header(2),
                ContentRow::Hit(2),
                ContentRow::Hit(3),
            ]
        );
    }

    #[test]
    fn a_fold_under_the_selection_never_strands_it() {
        let mut state = SidebarSearchState::new();
        state.open(FilesView::ContentSearch);
        state.content = vec![line("a.rs", 1, "x"), line("a.rs", 2, "y")];
        state.refresh_rows();
        state.selected = 2;
        state.toggle_file("a.rs");
        assert_eq!(state.selected, 0, "the vanished row's own header");
        assert!(state.selected_line().is_none());
    }

    /// The reported defect, at the state level: a fresh question must
    /// not append under the old one's lines.
    #[test]
    fn a_fresh_question_empties_the_standing_answer_but_keeps_folds() {
        let mut state = SidebarSearchState::new();
        state.open(FilesView::ContentSearch);
        state.content = vec![line("a.rs", 1, "old"), line("b.rs", 2, "old")];
        state.refresh_rows();
        state.toggle_file("a.rs");
        state.selected = 1;
        state.begin_query();
        assert!(state.content.is_empty());
        assert!(state.rows.is_empty());
        assert_eq!(state.selected, 0);
        assert!(
            state.collapsed.contains("a.rs"),
            "the fold outlives the answer"
        );
        // The next answer folds a.rs from its first line.
        state.content = vec![line("a.rs", 7, "new"), line("c.rs", 8, "new")];
        state.refresh_rows();
        assert_eq!(
            state.rows,
            vec![
                ContentRow::Header(0),
                ContentRow::Header(1),
                ContentRow::Hit(1)
            ]
        );
    }

    #[test]
    fn scrolling_keeps_the_selection_inside_the_viewport() {
        let mut state = SidebarSearchState::new();
        state.open(FilesView::FileSearch);
        state.files = (0..50).map(|n| hit(&format!("f{n:02}.rs"))).collect();
        state.selected = 49;
        state.move_selection(0, 90.0);
        assert!(
            state.scroll + 90.0 >= 50.0 * ROW_H,
            "the last row is visible"
        );
        state.selected = 0;
        state.move_selection(0, 90.0);
        assert_eq!(state.scroll, 0.0, "back at the top");
    }
}
