//! The help page: markdown generated in memory and shown as a document
//! of Oryx's own, so help is searchable, themed and scrollable. The
//! shortcut tables are built from `keymap::SHORTCUTS`, the dispatch
//! truth, and only the surrounding prose is authored, so the page can
//! never drift from what the keys do. Nothing lands on disk, and with
//! no file behind it the page cannot be edited, saved or reloaded.

use crate::input::keymap;

/// The page an empty launch shows in the document area: how to open a
/// file, where the folder sidebar is, where the settings are (a fresh
/// install on a scaled screen starts there), where the shortcuts are,
/// where the documentation is, and one tip from the list, the one at
/// `tip` in the rotation. No file stands behind it, so it cannot be
/// edited or saved, and the first file opened replaces it.
pub fn welcome(tip: usize) -> String {
    format!(
        "# Oryx\n\n\
         Press `{}` to open a file, or `{}` to start a markdown note.\n\n\
         `{}` shows or hides the folder sidebar, where you browse and open files.\n\n\
         `{}` opens the settings: fonts, sizes and the interface scale, \
         if the page looks too small or too large on this screen.\n\n\
         `{}` lists the shortcuts.\n\n\
         You can also drag and drop a file here.\n\n\
         Please refer to the full documentation on \
         [GitHub](https://github.com/wmahfoudh/oryx).\n\n{}",
        keymap::display("Ctrl+O"),
        keymap::display("Ctrl+M"),
        keymap::display("Ctrl+Shift+B"),
        keymap::display("Ctrl+,"),
        keymap::display("F1"),
        tip_block(tip),
    )
}

/// The link under a tip that asks for the next one; the app catches it
/// before any link reaches the browser.
pub const NEXT_TIP_LINK: &str = "tip:next";

/// The GitHub alert a tip renders as. Tip for something the reader can
/// do, Note for how Oryx behaves, Warning where work is at stake; the
/// other two kinds would be forced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipKind {
    Tip,
    Note,
    Warning,
}

impl TipKind {
    fn marker(self) -> &'static str {
        match self {
            TipKind::Tip => "TIP",
            TipKind::Note => "NOTE",
            TipKind::Warning => "WARNING",
        }
    }
}

/// One entry of the tips list: markdown, one paragraph, the keys in
/// code spans as the keymap spells them, so `keymap::display` can turn
/// Ctrl into Cmd on macOS when the page is built.
pub struct Tip {
    pub kind: TipKind,
    pub text: &'static str,
}

/// The tip at `index` in the rotation as its alert block, the link to
/// the next one under it against the right edge, the way a "more" link
/// sits under a card. The index wraps, so any launch count works.
pub fn tip_block(index: usize) -> String {
    let tip = &TIPS[index % TIPS.len()];
    format!(
        "> [!{}]\n> {}\n\n<p align=\"right\">\n\n[More tips...]({NEXT_TIP_LINK})\n\n</p>\n",
        tip.kind.marker(),
        keymap::display(tip.text)
    )
}

/// Every feature of Oryx, one tip each, in the order the welcome page
/// rotates through them. A dependency of every release: each tip is
/// checked still true against what the release changed, and each new
/// feature worth a tip gets one. Written as one person telling another
/// what to press, what happens and why it helps.
pub const TIPS: &[Tip] = &[
    Tip {
        kind: TipKind::Tip,
        text: r#"Press `Ctrl+O` to open a file. The dialog opens in the folder of the file you are reading, or in the folder the sidebar shows."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"You can drop a file on the window to open it. Drop a folder instead, and Oryx opens the sidebar on it."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+N` creates a new file. Oryx will ask you to save it first, so it knows the file's type and can apply the right syntax highlighting as you type."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"`Ctrl+M` starts a markdown note right away, without any dialog. The note has no file on disk until you save it with `Ctrl+S`, and Oryx will ask before letting you close it."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+S` saves your work. Oryx only writes the lines you changed, and each line keeps its own ending, so a file with Windows line endings stays that way."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+Shift+S` saves the file you are editing under a new name. From then on you are working on the new file, and the old one stays as it was."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"If the file you are reading changes on disk, Oryx reloads it by itself, as long as you have no unsaved edits. `F5` or `Ctrl+R` reloads whenever you want."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+Shift+R` reloads the file and fetches its remote images again. You will rarely need it, because a cached image older than a day is refreshed by itself."#,
    },
    Tip {
        kind: TipKind::Warning,
        text: r#"If the file changes on disk while you have unsaved edits, Oryx keeps your edits and shows you a notice. Save your version under another name with `Ctrl+Shift+S` if you want to keep both."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"If you close, quit or reload with unsaved changes, Oryx asks first. Press `S` to save, `D` to discard, or `Escape` to keep editing."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"From a terminal, `oryx file.md` opens a file, and `oryx notes/` starts Oryx with the sidebar on that folder."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Start Oryx with `oryx --theme nord file.md` to use a theme for this session only. Your usual theme stays as it is."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Run `oryx --register` once and your files will open with Oryx from the file manager, with the right icons."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Up` and `Down` scroll one line at a time. `Page Up`, `Page Down`, `Space` and `Shift+Space` scroll a whole page."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Home` takes you to the top of the document and `End` to the bottom, however long the document is."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"After you follow a link, a footnote or an outline entry, `Alt+Left` takes you back to where you were reading, one jump at a time."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+Shift+B` shows or hides the sidebar, where you browse folders and open files."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"The sidebar has two tabs, the files and the outline of the document's headings. Press `Ctrl+Tab` to switch between them."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Press `Left` to move the keyboard to the sidebar and `Right` to come back to the document. In the sidebar, `Up` and `Down` move the selection and `Enter` opens what is selected."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"The outline follows you as you read, folds its branches, and jumps when you click an entry. For a book, it is the table of contents."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+Plus` and `Ctrl+Minus` zoom in and out, and `Ctrl+0` brings the size back. You can also hold `Ctrl` and turn the mouse wheel."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"On a touch screen, swipe to scroll and let go while moving to keep it going. Tap to click, and pinch with two fingers to zoom."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"Oryx remembers the window size, the theme, the sidebar and the last folder from one run to the next, so it opens the way you left it."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"While Oryx is open, each file keeps its place when you switch between files. A file you left in the editor comes back in the editor, at the same spot."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+F` opens the search bar and finds text as you type. Press `Escape`, or click in the document, to close it."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"The search is smart about capitals. Type `oryx` and it matches Oryx and ORYX too; type `Oryx` with a capital and it matches exactly that."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Once you have searched with `Ctrl+F`, `F3` jumps to the next match and `Shift+F3` to the previous one."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"To search with regular expressions, press `Ctrl+F` to open the search bar, then `Alt+R` or the `.*` button at its end. Capture groups, backreferences and lookarounds all work."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"A search looks through the styling, so `fast viewer` is found even where it was written as `**fast** *viewer*`. A match can span a wrapped line too."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"You can search a big file while it is still loading. The whole document is searchable from the first second."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"In the editor, `Ctrl+H` adds a replace field under the search box. `Enter` replaces the current match and moves on, `Ctrl+Enter` replaces them all."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"A replace-all is one step. A single `Ctrl+Z` brings every replaced match back."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"When searching with regular expressions in the editor, the replacement can reuse what the search captured. Search for `(\w+)/(\w+)` and replace with `$2/$1` to swap the two sides of every pair."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"In regular expression mode, `\n` stands for a line break, in the search and in the replacement. That lets you join lines, collapse blank lines, or add a line after every line."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Looking for the task markers you left behind? Switch the search to regular expressions with `Alt+R` and search for `TODO|FIXME`."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"To find spaces left at the end of lines, switch the search to regular expressions with `Alt+R` and search for ` +$`."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"A word typed twice in a row, like "the the", is found by searching for `\b(\w+) \1\b` with regular expressions on."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+A` selects the whole document, instantly, however big the file is."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Double click a word to select it. Triple click selects the whole paragraph, or the code line, or the table cell."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+C` copies your selection as plain text. `Ctrl+Shift+C` copies the markdown behind it, exactly as it was written."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+E` opens the editor, with a caret and the usual keys, and `Escape` takes you back to reading."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"In the editor, a markdown file shows you its source with the markers visible. Code and text files are edited right on the page."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"While you are editing, the window title says `editing` and a thin line in the selection color runs along the top of the page. A dot next to the file name means you have unsaved changes."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"In the editor, `Ctrl+X` cuts and `Ctrl+V` pastes, as in any text editor."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+Z` undoes and `Ctrl+Shift+Z` or `Ctrl+Y` redoes. Every editing shortcut is a single step, so one `Ctrl+Z` takes it back whole."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Select some text, or just leave the caret on a word, and press `Ctrl+B` for bold, `Ctrl+I` for italic or `` Ctrl+` `` for code. Press the same key again to remove it."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+K` turns the selected text into a link. You can also paste a web address over a selection and Oryx will make it a link."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Select a few lines and press `Alt+-` for a bullet list, `Alt+1` for a numbered list or `Alt+X` for a task list. The same key again removes it."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"To quote a few lines, select them and press `Alt+.`. Press it again to remove the quote."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+1` to `Ctrl+6` turn the current line into a heading of that level. Press the same level again to make it a plain line."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"In the editor, `Ctrl+L` ticks or unticks the task box on the current line."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"You can tick a task box by clicking it on the page, without opening the editor. Nothing else moves, `Ctrl+Z` undoes it and `Ctrl+S` saves it."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Alt+Up` and `Alt+Down` move the current line, or the selected lines, up and down."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+Shift+D` duplicates the current line or the selected lines, and `Ctrl+Shift+K` deletes them."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+/` comments out the current line or the selected lines, in code as in markdown. Press it again to bring them back."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"When text is selected, typing a bracket or a quote wraps the selection instead of replacing it. In a markdown file, `*`, `_` and the backtick do the same."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Enter` carries a list on: the next bullet or number, an unchecked task, another quoted line. Press `Enter` on an empty item to end the list."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Tab` indents and `Shift+Tab` removes an indent, on every selected line at once. With the caret at a list marker, `Tab` nests the item under the one above."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"In the editor, `Ctrl+Left` and `Ctrl+Right` move a word at a time, and `Ctrl+Backspace` and `Ctrl+Delete` delete a whole word."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"In the editor, `Ctrl+Home` and `Ctrl+End` jump to the start and the end of the file."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"Books cannot be edited, and neither can a file whose text Oryx could not read cleanly, since it could not write it back as it was. A small notice in the corner tells you when that is the case."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+T` opens the theme browser. Move through it with the arrows and the document takes each theme as you go. `Enter` keeps the one you are on, `Escape` brings back the one you had."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Oryx comes with over thirty themes, and each is a plain TOML file with 51 color roles, one for every kind of element on the page."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"In the theme browser, each row has three small icons at its right: one opens the theme in the color editor, one duplicates it and one deletes it. A double click on the name renames it."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"The theme editor changes any color of a theme with a color picker while the document restyles in front of you. Editing a bundled theme makes a copy, so the original stays."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Would you like a theme of your own? Give one of the TOML theme files to an AI assistant, along with a picture whose colors you like, and ask for a matching theme."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+,` opens the settings: the body font and the code font, their sizes, and the interface scale."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Does the page look too small or too large on your screen? The interface scale in the settings adjusts it from -50% to +100%, and Oryx remembers your choice."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+J` justifies the text so lines end on the same right edge, like print. Books start justified and markdown files ragged, and Oryx remembers your choice for each kind."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"If Oryx gets the reading direction of a book wrong, `Ctrl+D` cycles through automatic, right to left and left to right. The choice is remembered for that book."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+P` exports the document to PDF with your export settings, in one go. Headings become the PDF's outline and the fonts are embedded."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`Ctrl+Shift+P` lets you choose the export settings before exporting: theme, fonts, sizes, page size, orientation and page numbers."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"The export settings are separate from your reading settings. You can read in a dark theme at a large size and export in a light one at a small size, without switching back and forth."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"To force a page break in the PDF, write `\newpage` on a line of its own. On screen it shows as a dashed line. `<div style="page-break-after: always"></div>` does the same."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"When exporting, Oryx keeps a heading with the text below it, never cuts a line or an image in two, and keeps a table row whole unless the row is taller than the page."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"To break a line inside a paragraph, end it with two spaces or a backslash. A plain line ending simply joins with the next line."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Write `==text==` or `::text::` to highlight a phrase, the way a marker pen would."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`H~2~O` lowers the 2 into a subscript, and `x^2^` raises the 2 into a superscript."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Emoji shortcodes like `:tada:` show as the emoji, and pasted emoji show as they are."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Oryx tidies your punctuation as it renders: straight quotes become curly, `--` and `---` become dashes, and three dots become an ellipsis."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"For a definition list, write the term on its own line and each definition on the lines below, starting with a colon."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Start a quote with `[!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]` or `[!CAUTION]` on its first line and it renders as a GitHub alert, like this one."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"A heading can carry its own anchor in braces at the end, like `{#custom-id}`, and a link to `#custom-id` will jump to it."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Footnotes are numbered in the order you use them, whatever their labels say, and gather at the foot of the document. The number there links back to the text, and `Alt+Left` returns you."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Write `*[W3C]: World Wide Web Consortium` anywhere in the file and every W3C in the document gets a dotted underline. Rest the mouse on it to see the expansion."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Math works in all four GitHub notations: `$...$`, `$$...$$`, a `math` fence and the backtick form. Prices like `$5-$10` are left alone."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"A YAML block between two `---` lines at the top of a file shows as a small metadata panel above the document."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Tables keep each column's alignment, shade every other row and wrap long cells, so a wide table never runs off the page."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"A fenced code block with a language name gets syntax colors, and over a hundred languages are recognized."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Open a source file directly and Oryx renders the whole file with syntax colors, as one document."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"A `Dockerfile` or a `Makefile` is recognized by its name. Any other text file simply opens in the code font."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"A link to another file opens it in Oryx, and `README.md#install` takes you straight to that section. A web address opens in your browser."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"Remote images and badges are fetched in the background and kept on disk, so a file full of badges comes up at once the second time, even offline. `oryx --clear-cache` empties that cache."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"A `<details>` section folds its content under its summary line. Search still looks inside a closed one, and jumping to a match unfolds it."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Oryx renders the HTML that GitHub allows in READMEs: tables, headings, lists, images at a set width, rows of clickable badges, and inline tags like `<kbd>`, `<mark>`, `<sub>` and `<sup>`."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`<p align="center">` centers a block and `<p align="right">` puts it against the right edge, as on GitHub. The link under this tip is one."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Oryx opens EPUB, FB2, MOBI and AZW3 books as one continuous document, in the theme you are using rather than the book's own styling. The first chapters show at once and the rest loads while you read."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"A book reopens where you stopped reading, even after you close Oryx. Other files open at the top."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"Arabic and Hebrew read from right to left, paragraph by paragraph, in fonts made for them, whatever body font you chose."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"The window title shows a book's title and its format. That helps when you have the same book in several formats."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"A comic, CBZ or CBR, opens as a vertical strip. `Ctrl+Minus` shows one whole page at a time, then two pages side by side like an open book, and `Ctrl+0` shows the whole page from wherever you are."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"In a comic's page views, `Up`, `Down` and `Space` turn the pages."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"For a manga, or any comic that reads from right to left, `Ctrl+D` flips the order of the two pages side by side, and Oryx remembers it for that comic."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"The examples folder installed with Oryx has sample documents, a math sample and *The Adventures of Sherlock Holmes*, so you can try the book reader right away."#,
    },
    Tip {
        kind: TipKind::Tip,
        text: r#"`F1` lists every shortcut on a page of its own. Press `F1` again, or `Escape`, to close it."#,
    },
    Tip {
        kind: TipKind::Note,
        text: r#"`Escape` closes whatever is open: a dialog, the search bar, a selection, the editor. When nothing is open, it quits Oryx."#,
    },
];

/// The generated page.
pub fn page() -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(4096);
    let _ = write!(out, "# Oryx v{} Shortcuts\n\n", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(
        out,
        "Press {} or {} to close this. \
         Please refer to the full documentation on \
         [GitHub](https://github.com/wmahfoudh/oryx).",
        keymap::display("F1"),
        keymap::display("Escape"),
    );
    let mut section = "";
    for row in keymap::SHORTCUTS {
        if row.section != section {
            section = row.section;
            let _ = write!(out, "\n## {section}\n\n| Shortcut | Action |\n|---|---|\n");
        }
        let _ = writeln!(
            out,
            "| {} | {} |",
            code(&keymap::display(row.keys)),
            row.action
        );
    }
    out.push_str("\n## While editing\n\n");
    let _ = writeln!(
        out,
        "The arrows, `Home`, `End`, `Page Up` and `Page Down` move the caret. \
         `{}` / `{}` jump by word, `{}` / `{}` jump to the ends of the file, and \
         `{}` / `{}` delete by word. Typing replaces a selection, and `{}` followed \
         by typing replaces the whole file. `Enter` keeps the line's indentation, \
         and in a markdown file it continues lists and quotes. `Tab` indents and \
         `Shift+Tab` removes an indent, over every selected line at once; at a \
         list marker, `Tab` nests the item.",
        keymap::display("Ctrl+Left"),
        keymap::display("Ctrl+Right"),
        keymap::display("Ctrl+Home"),
        keymap::display("Ctrl+End"),
        keymap::display("Ctrl+Backspace"),
        keymap::display("Ctrl+Delete"),
        keymap::display("Ctrl+A"),
    );
    out.push_str(
        "\nClosing, quitting or reloading with unsaved changes asks first: \
         `S` saves, `D` discards, `Escape` keeps editing, or the arrows and `Enter` \
         pick one of the three.\n",
    );
    out.push_str("\n## Sidebar\n\n");
    let _ = writeln!(
        out,
        "`{}` moves the keys to the sidebar and `{}` brings them back to the document. \
         In the sidebar, `Up` and `Down` move the selection, `Enter` opens the selected \
         file or folder (the `..` row goes up), or jumps to the selected heading on the \
         outline tab, and `{}` switches between the files and the outline.",
        keymap::display("Left"),
        keymap::display("Right"),
        keymap::display("Ctrl+Tab"),
    );
    out.push_str("\n## Mouse and touch\n\n");
    let _ = writeln!(
        out,
        "A double click selects the word, a triple click the paragraph or the code line. \
         The wheel scrolls, and with `{}` held it zooms; dragging the scrollbar or its \
         track jumps. A file dropped onto the window opens, and a dropped folder opens \
         the sidebar on it. On a touch screen, swiping scrolls with momentum, tapping \
         clicks, and a two-finger pinch zooms.",
        keymap::display("Ctrl"),
    );
    out
}

/// A chord as a code span. A chord holding a backtick takes the
/// double-backtick form, the only one markdown reads whole.
fn code(chord: &str) -> String {
    if chord.contains('`') {
        format!("`` {chord} ``")
    } else {
        format!("`{chord}`")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shortcut_stands_on_the_page() {
        let page = page();
        for row in keymap::SHORTCUTS {
            assert!(
                page.contains(&keymap::display(row.keys)),
                "the page names {}",
                row.keys
            );
            assert!(
                page.contains(row.action),
                "the page describes {}",
                row.action
            );
        }
        for section in ["Files", "Navigation", "Edit", "Export"] {
            assert!(page.contains(section), "the page has a {section} caption");
        }
    }

    /// The settings dialog sets the fonts, the sizes and the interface
    /// scale; the row that opens it says so, since a fresh install on a
    /// scaled screen starts there.
    #[test]
    fn the_settings_row_names_the_interface_scale() {
        let row = keymap::SHORTCUTS
            .iter()
            .find(|row| row.keys == "Ctrl+,")
            .expect("the settings row");
        assert!(row.action.contains("scale"), "{}", row.action);
    }

    #[test]
    fn the_arrow_row_says_where_the_keys_go() {
        let row = keymap::SHORTCUTS
            .iter()
            .find(|row| row.keys == "Left / Right")
            .expect("the arrow row");
        assert_eq!(row.action, "Move to the sidebar / to the document");
    }

    #[test]
    fn the_sidebar_paragraph_says_what_enter_does_there() {
        let page = page();
        assert!(page.contains("## Sidebar"), "{page}");
        assert!(page.contains("opens the selected file or folder"), "{page}");
        assert!(page.contains("outline"), "{page}");
    }

    #[test]
    fn the_welcome_page_points_at_the_settings_and_the_documentation() {
        let page = welcome(0);
        assert!(
            page.contains(&format!(
                "`{}` opens the settings",
                keymap::display("Ctrl+,")
            )),
            "{page}"
        );
        assert!(page.contains("interface scale"), "{page}");
        assert!(
            page.contains("[GitHub](https://github.com/wmahfoudh/oryx)"),
            "{page}"
        );
    }

    #[test]
    fn the_welcome_page_names_the_four_chords_and_nothing_unbound() {
        let page = welcome(0);
        for keys in ["Ctrl+O", "Ctrl+Shift+B", "Ctrl+,", "F1"] {
            assert!(
                page.contains(&format!("`{}`", keymap::display(keys))),
                "the welcome page names {keys}"
            );
        }
        let bound: Vec<String> = keymap::SHORTCUTS
            .iter()
            .map(|row| keymap::display(row.keys))
            .collect();
        // The tip block at the end names its own keys; the lines above
        // it are the fixed page.
        let fixed = page.split("\n> [!").next().unwrap();
        for chord in fixed.split('`').skip(1).step_by(2) {
            assert!(
                bound.contains(&chord.to_string()),
                "the welcome page names {chord}, which the keymap does not bind"
            );
        }
        assert!(page.starts_with("# Oryx\n"), "the page opens on the name");
        assert!(fixed.lines().count() <= 14, "the page stays short: {page}");
    }

    /// The code spans of a tip that name keys: a chord with a modifier,
    /// or a function key.
    fn chords_of(text: &str) -> Vec<String> {
        pulldown_cmark::Parser::new(text)
            .filter_map(|event| match event {
                pulldown_cmark::Event::Code(span) => Some(span.to_string()),
                _ => None,
            })
            .filter(|span| {
                span.starts_with("Ctrl+")
                    || span.starts_with("Alt+")
                    || span.starts_with("Shift+")
                    || (span.starts_with('F') && span[1..].chars().all(|c| c.is_ascii_digit()))
            })
            .collect()
    }

    /// The chords the keymap binds, each entry split into its keys:
    /// "F5 / Ctrl+R" binds two, "Ctrl+1 to Ctrl+6" names its two ends.
    fn bound_chords() -> Vec<String> {
        keymap::SHORTCUTS
            .iter()
            .flat_map(|row| {
                row.keys
                    .split(" / ")
                    .flat_map(|part| part.split(", "))
                    .flat_map(|part| part.split(" to "))
                    .map(str::to_string)
            })
            .collect()
    }

    /// The text editor's own movement keys, documented in the README
    /// and handled by the editor rather than the keymap.
    const EDITOR_KEYS: &[&str] = &[
        "Ctrl+Left",
        "Ctrl+Right",
        "Ctrl+Home",
        "Ctrl+End",
        "Ctrl+Backspace",
        "Ctrl+Delete",
        "Shift+Tab",
    ];

    #[test]
    fn every_chord_a_tip_names_is_bound() {
        let bound = bound_chords();
        for tip in TIPS {
            for chord in chords_of(tip.text) {
                assert!(
                    bound.contains(&chord) || EDITOR_KEYS.contains(&chord.as_str()),
                    "a tip names {chord}, which nothing binds: {}",
                    tip.text
                );
            }
        }
    }

    #[test]
    fn every_keymap_entry_has_a_tip() {
        let named: Vec<String> = TIPS.iter().flat_map(|tip| chords_of(tip.text)).collect();
        for chord in bound_chords() {
            let plain = !chord.contains('+') && !chord.starts_with('F');
            assert!(plain || named.contains(&chord), "no tip names {chord}");
        }
        for key in [
            "Up", "Down", "Left", "Right", "Home", "End", "Space", "Escape",
        ] {
            assert!(
                TIPS.iter()
                    .any(|tip| tip.text.contains(&format!("`{key}`"))),
                "no tip names {key}"
            );
        }
    }

    #[test]
    fn a_tip_block_is_an_alert_of_its_kind_with_the_link_right_under_it() {
        let tip = &TIPS[0];
        let block = tip_block(0);
        assert!(block.starts_with("> [!TIP]\n> "), "{block}");
        assert!(block.contains(&keymap::display(tip.text)), "{block}");
        assert!(
            block.ends_with(&format!(
                "\n\n<p align=\"right\">\n\n[More tips...]({NEXT_TIP_LINK})\n\n</p>\n"
            )),
            "{block}"
        );
        let note = TIPS.iter().position(|t| t.kind == TipKind::Note).unwrap();
        assert!(tip_block(note).starts_with("> [!NOTE]\n> "));
        let warning = TIPS
            .iter()
            .position(|t| t.kind == TipKind::Warning)
            .unwrap();
        assert!(tip_block(warning).starts_with("> [!WARNING]\n> "));
    }

    #[test]
    fn the_rotation_wraps_and_the_page_carries_the_tip() {
        assert_eq!(
            tip_block(TIPS.len()),
            tip_block(0),
            "past the end, the first again"
        );
        assert_eq!(tip_block(TIPS.len() + 3), tip_block(3));
        let page = welcome(5);
        assert!(
            page.ends_with(&tip_block(5)),
            "the tip closes the page: {page}"
        );
        assert_ne!(welcome(5), welcome(6), "each index its own tip");
    }

    #[test]
    fn every_tip_is_short_and_ends_as_a_sentence() {
        for tip in TIPS {
            assert!(tip.text.len() <= 320, "too long: {}", tip.text);
            assert!(tip.text.ends_with('.'), "no full stop: {}", tip.text);
            assert!(!tip.text.contains('\n'), "one paragraph: {}", tip.text);
        }
        assert!(TIPS.len() >= 50, "the list covers Oryx: {}", TIPS.len());
    }

    #[test]
    fn the_mouse_paragraph_names_dropping_and_the_wheel_zoom() {
        let page = page();
        assert!(page.contains("dropped onto the window opens"));
        assert!(page.contains(&format!("with `{}` held it zooms", keymap::display("Ctrl"))));
        assert!(welcome(0).contains("drag and drop a file here"));
    }

    #[test]
    fn the_page_names_the_version_and_the_project_homes() {
        let page = page();
        assert!(
            page.contains(env!("CARGO_PKG_VERSION")),
            "the running version stands on the page"
        );
        assert!(page.contains("https://github.com/wmahfoudh/oryx"));
        assert!(!page.contains("codeberg"), "GitHub is the only host named");
    }

    /// The inline-code chord holds a backtick, which a single-backtick
    /// code span cannot carry: the page writes it in the double-backtick
    /// form and markdown reads it back whole.
    #[test]
    fn the_backtick_chord_survives_as_a_code_span() {
        let page = page();
        let chord = keymap::display("Ctrl+`");
        assert!(page.contains(&format!("`` {chord} ``")), "{page}");
        assert!(
            !page.contains(&format!("`{chord}`")),
            "a single-backtick span breaks on the chord: {page}"
        );
        let spans: Vec<String> = pulldown_cmark::Parser::new(&page)
            .filter_map(|event| match event {
                pulldown_cmark::Event::Code(text) => Some(text.to_string()),
                _ => None,
            })
            .collect();
        assert!(
            spans.iter().any(|span| span == &chord),
            "markdown reads the chord back whole: {spans:?}"
        );
    }

    #[test]
    fn the_welcome_page_says_the_sidebar_key_shows_and_hides_the_panel() {
        let welcome = welcome(0);
        assert!(
            welcome.contains("shows or hides the folder sidebar"),
            "the key toggles, and the page says so: {welcome}"
        );
    }
}
