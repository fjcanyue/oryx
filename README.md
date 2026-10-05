<div align="center">

<!-- WORDMARK: the logo and "Oryx" side by side, on a rounded rectangle. wordmark-dark.svg has light text on a dark rectangle (dark mode), wordmark-light.svg has dark text on a light rectangle (light mode). -->
<h1 align="center">
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="images/wordmark-dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="images/wordmark-light.svg">
  <img src="images/wordmark-dark.svg" alt="Oryx" width="260">
</picture>
</h1>

**A fast, native markdown editor and reader.**<br>
It also edits code, opens ebooks and comics, on Linux, Windows and macOS.

<a href="https://github.com/wmahfoudh/oryx/releases"><img alt="Release" src="https://img.shields.io/github/v/release/wmahfoudh/oryx?style=for-the-badge&label=RELEASE&color=purple"></a>
<a href="https://www.rust-lang.org"><img alt="Language" src="https://img.shields.io/badge/LANGUAGE-RUST-orange?style=for-the-badge&logo=rust&logoColor=white"></a>
<a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/LICENSE-GPL--3.0-darkgreen?style=for-the-badge"></a>

[Install](#install) · [Features](FEATURES.md) · [Shortcuts](SHORTCUTS.md) · [Changelog](CHANGELOG.md)

<!-- GIF: the main demo (oryx-main.txt), about 47 s, 2000x1200, under 5 MB. -->
<img src="images/oryx.gif" alt="Oryx demo: an 8 MB file, editing, themes, then code, an ebook and a comic">

</div>

Oryx is a markdown editor and reader, made for a keyboard workflow. It also edits code and reads ebooks and comics. Oryx opens a file in reading mode, and `Ctrl+E` switches between reading and editing. With no menus and no toolbars, it suits those who want to focus on the content and on writing. Oryx is a native app with its own rendering engine (no browser inside), built for speed and beauty: an 8 MB markdown file shows in **milliseconds**.

## Install

<a href="https://apps.microsoft.com/detail/9NQGHNSJF3VB"><picture><source media="(prefers-color-scheme: dark)" srcset="https://get.microsoft.com/images/en-us%20light.svg"><img alt="Get it from Microsoft" src="https://get.microsoft.com/images/en-us%20dark.svg" width="200"></picture></a>

Every package is on the [releases page](https://github.com/wmahfoudh/oryx/releases). Pick the one for your system:

- **Windows**: the Microsoft Store (badge above), the MSI installer, or the zip to install in your user folder without administrator rights.
- **macOS**: `brew install --cask wmahfoudh/tap/oryx`, or the `.dmg`. The app is not signed, so macOS asks you to approve it once ([how](INSTALL.md#macos)).
- **Debian and Ubuntu**: `sudo apt install ./oryx-editor_*_amd64.deb`
- **Fedora and openSUSE**: `sudo dnf install ./oryx-editor-*.x86_64.rpm` (on openSUSE, `zypper` in place of `dnf`)
- **Arch Linux**: `sudo pacman -U oryx-editor-bin-*.pkg.tar.zst`
- **Any Linux**: the AppImage, or the tarball with its install script.

On Linux, the command is `oryx`. On Windows and macOS, the installers do not add a terminal command (for the moment): [INSTALL.md](INSTALL.md#from-a-terminal) says how to call Oryx there. [INSTALL.md](INSTALL.md) also covers the requirements and building from source.

## What Oryx does

### Fast with any file size

When you open a file, Oryx lays out only the first screens and shows them. The rest of the file is read and laid out in the background, on all CPU cores, while you read. Scrolling does not make memory grow, because Oryx draws only the part of the document around where you are. A window that is not doing anything uses no CPU. This is why Oryx runs the same on an old laptop with no graphics card. The [numbers](#performance) are measured before every release.

### Your files, where they are

Oryx opens files where they already are on your disk. There is no vault, no library and nothing to import. You open a file from the sidebar, with `Ctrl+O`, by dropping a file or a folder on the window, or from a terminal with `oryx notes.md`. The sidebar shows the folder you are in, and an outline of the open file.

Oryx opens files fast enough that it does not need tabs: you save, click another file in the sidebar, and when you come back to the first one, it is at the line where you left it. `Alt+Left` and `Alt+Right` go back and forward between the files you opened, like in a browser. To see two files at once, a middle click on a file in the sidebar, or `Ctrl+Enter` on the selected file, opens it in a second window.

<p align="center"><img src="images/sidebar.png" alt="The Oryx sidebar on a folder of notes, with a note open" width="800"></p>

### Markdown as GitHub shows it, and more

Oryx reads CommonMark and GitHub Flavored Markdown: tables, task lists, strikethrough, footnotes, alerts and math. It also renders the HTML that GitHub allows in a README, like centered blocks, badges and collapsible sections, so a README looks in Oryx the way it looks on GitHub. Remote images and badges are saved on disk: they still show when you are offline.

Oryx also reads the extended syntax of the Markdown Guide, which GitHub does not: highlight, subscript and superscript, definition lists and abbreviations. Math written in TeX is typeset with the STIX Two Math font, and it looks the same in the PDF. [SYNTAX.md](SYNTAX.md) shows every construct, and `F1` opens the same reference inside Oryx.

<p align="center"><img src="images/markdown.png" alt="Oryx rendering an alert, a table and typeset math" width="800"></p>

### Themes and PDF export

Oryx comes with 33 themes, 11 of them made for Oryx. Each theme sets 51 colors, so every markdown element can have its own color. `Ctrl+T` opens the list of themes: the arrow keys show each theme on your document, `Enter` keeps it and `Escape` goes back to the one you had. The theme editor changes any color with a color picker while the page updates, and saves your changes in a copy of the theme.

`Ctrl+P` exports the document to PDF. The PDF is made by the same engine that draws the page, so it has the same layout, fonts, syntax colors and math, and the headings are converted to PDF outlines. Oryx avoids cutting a line, an image or a table row across two pages, and does not leave a heading alone at the bottom of a page. The export has its own theme, fonts and page size, set with `Ctrl+Shift+P`, so you can read in a dark theme and export in a light one without switching each time.

<p align="center"><img src="images/themes.png" alt="The Oryx theme browser open over a document" width="800"></p>

<p align="center"><img src="images/pdf.png" alt="A PDF exported by Oryx" width="800"></p>

### Text and code, for quick edits

Oryx opens text files, and source files with syntax colors for more than a hundred file extensions. Files without an extension, like `.bashrc` or a script that starts with `#!`, are recognized too. From a terminal, `oryx main.rs:412:10` opens a file at line 412, column 10, and `git diff | oryx` shows a diff in color.

`Ctrl+E` lets you edit directly on the page, with the line tools of a code editor: move, duplicate or delete lines, comment them out in the style of the language, find and replace with regular expressions, go to a line, and show line numbers. Saving keeps the line endings of the file, so a Windows file stays a Windows file.

For everyday text files, Oryx can take the place of Notepad or Notepad++, on Linux and macOS too. Oryx is not designed to compete with code editors like VS Code or Zed: it has no plugins and no git.

<p align="center"><img src="images/code.png" alt="Editing code in Oryx with line numbers and regex find and replace" width="800"></p>

### Ebooks and comics

Oryx opens EPUB, MOBI, AZW3 (Kindle) and FB2 books. It shows them in your theme and fonts instead of the book's own style, and justifies the text like a printed book (`Ctrl+J` turns it off). The sidebar shows the book's table of contents, and a book opens again where you stopped reading, even after you close Oryx. To try it, *The Adventures of Sherlock Holmes* comes with Oryx in the examples folder.

Arabic and Hebrew books read from right to left. Oryx finds the direction of each paragraph from its text, so in a book that mixes English and Arabic, each paragraph reads in its own direction. Two fonts made for these scripts are built in: Amiri for Arabic and David Libre for Hebrew.

Oryx also opens CBZ and CBR comics. A comic starts as a vertical strip of pages, which suits webtoons. `Ctrl+Minus` switches to one page per screen, and again to two pages side by side, like an open book. `Ctrl+D` reverses the page order for manga.

<p align="center"><img src="images/book-arabic.png" alt="An Arabic book in Oryx, justified from right to left" width="800"></p>

<p align="center"><img src="images/comic.jpg" alt="A comic in Oryx, two pages side by side" width="800"></p>

## Keyboard

Oryx has no menus and no toolbar. Everything is done with the keyboard and mouse, using the shortcuts most apps already use: `Ctrl+F` to find, `Ctrl+S` to save, `Ctrl+B` for bold. You do not need to learn a new way of typing, as with Vim, and the mouse works too.

A few rules hold everywhere: `Escape` closes whatever is open (when nothing is open, it quits Oryx), a formatting shortcut pressed again removes the formatting, and in the dialogs the arrow keys move and change values. `F1` shows every shortcut and the full markdown syntax, and can be searched. The shortcuts and the command line options are also listed in [SHORTCUTS.md](SHORTCUTS.md).

<!-- GIF: the editor tricks (editor-tricks.txt), one part or the three, under 5 MB. -->
<img src="images/editor-tricks.gif" alt="Editing markdown in Oryx with the keyboard only: lists, tasks, headings, links, pictures, moving lines and find and replace">

| Key | What it does |
|---|---|
| `Ctrl+E` | Switch between reading and editing |
| `Ctrl+O` | Open a file |
| `Ctrl+Shift+B` | Show or hide the sidebar |
| `Ctrl+F` | Find in the document |
| `Ctrl+G` | Go to a line:column |
| `Alt+Left` / `Alt+Right` | Back and forward |
| `Ctrl+T` | Choose a theme |
| `Ctrl+,` | Settings: fonts, sizes, line numbers, word count, autosave |
| `Ctrl+P` | Export to PDF |
| `Ctrl+Shift+P` | PDF export settings |
| `F1` | All shortcuts and the markdown syntax |

On macOS, use `Cmd` where this page says `Ctrl`, and `Cmd+[` / `Cmd+]` to go back and forward.

## Is Oryx for you?

Oryx is for people who want to read and write markdown fast, with the keyboard, in one window. It shows a file formatted, in the theme you chose, as soon as you open it, and it stays fast even on huge files. Oryx also opens and edits code, and reads books and comics, in the same app.

Oryx suits work with LLMs and coding agents, which write a lot of markdown. It reloads a file that changed on disk as soon as you come back to the window, so an agent's plan or report shows formatted and up to date. `llm "your question" | oryx --as md` shows an answer piped from the terminal.

For other needs, other tools are a better fit:

- Linked notes, a graph of notes, plugins or sync: Obsidian.
- Editing inside the formatted text: Typora or MarkText.
- The source and the preview side by side: Ghostwriter, or VS Code with its preview.
- A full code editor with extensions, git and language servers: VS Code or Zed.
- A library to organize and convert ebooks: Calibre.
- Markdown in the terminal: Glow or Neovim.

## Performance

Every release is measured on my machine, a 2019 laptop with an Intel Core i7-8565U, 16 GB of memory, an NVMe drive and no dedicated graphics card. A release ships only if it is as fast as the last one, or slower for a reason I can explain. The tables below come from the last measure.

The first table is about time. "First screen" is the time from reading the file to showing it, and it stays at 40 ms whatever the size of the file. The next three columns are work Oryx does in the background while you read: reading the markdown of the rest of the file (a source file has none, hence 0 ms), laying out the rest of the file, and coloring the code. "PDF export" is the export of the whole file.

| File | First screen | Rest read | Rest laid out | Code colors | PDF export |
|---|---|---|---|---|---|
| 1 MB markdown | 40 ms | 31 ms | 0.18 s | 0.65 s | 0.95 s |
| 1 MB source code | 40 ms | 0 ms | 0.11 s | 2.7 s | 0.84 s |
| 8 MB markdown | 40 ms | 256 ms | 1.5 s | 5.7 s | 8.8 s |
| 8 MB source code | 40 ms | 0 ms | 0.85 s | 24.2 s | 7.7 s |

The second table is about memory. "Once loaded" is what the file takes once all of this is done. "Highest" is the most it takes on the way there. "PDF export" is what an export adds while it runs.

| File | Once loaded | Highest | PDF export |
|---|---|---|---|
| 1 MB markdown | 22 MB | 34 MB | +10 MB |
| 1 MB source code | 11 MB | 12 MB | +11 MB |
| 8 MB markdown | 172 MB | 255 MB | +34 MB |
| 8 MB source code | 88 MB | 95 MB | +11 MB |

Books open the same way: the first chapters show at once, and the rest loads in the background. The third table gives the time before the first chapters of a book, or the first pages of a comic, show.

| Book | Format | Opens in |
|---|---|---|
| *The Adventures of Sherlock Holmes* | EPUB | 4 ms |
| A book of 300 chapters (1.6 MB) | FB2 | 27 ms |
| A book of 300 chapters (1.6 MB) | MOBI | 14 ms |
| A comic of 40 pages | CBZ | 1 ms |
| The same comic | CBR | 7 ms |

The 211 pages of *The Adventures of Sherlock Holmes* export to PDF in 0.8 s.

My goal is to make Oryx the fastest markdown editor and renderer. I also benchmark Oryx against other tools to improve it, but I don't publish those numbers: a comparison is best done by a third party. Independent benchmarks are welcome. The Oryx test files and the measures are in the repository:

```sh
cargo test --release --test perf -- --ignored --nocapture --test-threads=1      # the timings
cargo test --release --test perf_mem -- --ignored --nocapture --test-threads=1  # the memory
```

> [!NOTE]
> Oryx is not a markdown-to-PDF converter. Its export reproduces themed pages, at a few milliseconds per finished page whatever the document size. Raw conversion is a far faster job: a few milliseconds for a whole small file.

## Privacy

Oryx has no account and no telemetry. It does not report what you open, what you do, or that it is installed. It uses the network to download the images that a markdown file links to by URL, like the badges of a README. These images are kept on your disk so the file opens fast the next time, and `oryx --clear-cache` removes them. [PRIVACY.md](PRIVACY.md) lists everything Oryx keeps on your machine, and where.

## Limitations

- Oryx opens text files in UTF-8 only. A file in an older encoding, like Windows-1252 or Shift JIS, is refused rather than shown wrong. Binary files are refused too.
- Books protected by DRM and fixed-layout EPUB books do not open. CBR comics open when their pages are stored in the RAR archive without compression, which is the usual case, but not when the archive compresses them.
- Oryx renders the HTML that GitHub allows in a README, plus page breaks and aligned blocks. It does not apply CSS and does not render whole HTML pages.
- Typing works with any keyboard layout, but the input methods for Chinese, Japanese and Korean are not supported.
- On a file of several megabytes, the rest of the file is laid out and colored in the background, so a few lines can change color a moment after you reach them. A PDF export waits for this coloring to finish before it can start.
- On Linux, Oryx needs glibc 2.35 and OpenSSL 3, which recent distributions have ([the list](INSTALL.md#requirements)). On an older system, Oryx does not start.
- The Mac build is made on GitHub's Mac machines and tested by some nice Mac users (thank you!).

## Why Oryx exists

Oryx started as a personal project. I read and write a lot of markdown files, and I tried many of the free editors. Most take time to open, and slow down or crash on large files. Many are built on Electron or a browser engine, which makes them heavy. My ideal would have been a portable desktop app that shows formatted markdown, opens instantly on any machine, does not run a browser inside, and does not share my notes with somebody on the other side of the planet. A good-looking PDF export would have been a plus. The closest was Obsidian, but the vault constraint breaks the flow.

So Oryx began as a renderer, not as an editor. Most markdown editors show the source and then render it. I think a document should open formatted. A browser does not start by showing the raw HTML. Editing came later, as a consequence, then ebooks and comics, because the same engine could do them. Some ideas were tried and left out: reading PDF files would have added 5.5 MB to the app without doing better than the PDF readers that already exist. Git and AI agent integration will probably never come. A new feature has to keep Oryx `Fast and Beautiful`, or it stays out.

## Feedback

Bug reports and ideas are welcome in the [issues](https://github.com/wmahfoudh/oryx/issues). For a bug, the version (`F1`), your system, and if possible the file that shows the problem (or part of it) with some screenshots would be very helpful.

## Credits

Five fonts are built into Oryx: DejaVu Sans for text, Courier Prime for code, STIX Two Math for math, Amiri for Arabic and David Libre for Hebrew. DejaVu is under the DejaVu Fonts License, the other four under the SIL Open Font License. The settings (`Ctrl+,`) can switch the text and code fonts to any font installed on your system.

The sample book in [examples](examples/) is the [Standard Ebooks](https://standardebooks.org) edition of *The Adventures of Sherlock Holmes*, in the public domain and dedicated with CC0 by its producers.

<details>
<summary><b>Adapted theme palettes</b> (all MIT, with thanks to their authors)</summary>

- Dracula ([draculatheme.com](https://draculatheme.com))
- Nord ([nordtheme.com](https://www.nordtheme.com))
- Gruvbox dark and light ([morhetz/gruvbox](https://github.com/morhetz/gruvbox))
- Catppuccin Mocha and Latte ([catppuccin.com](https://catppuccin.com))
- Tokyo Night ([enkia/tokyo-night-vscode-theme](https://github.com/enkia/tokyo-night-vscode-theme))
- Solarized dark and light by Ethan Schoonover ([ethanschoonover.com/solarized](https://ethanschoonover.com/solarized))
- One Dark ([atom](https://github.com/atom/atom))
- Everforest dark and light ([sainnhe/everforest](https://github.com/sainnhe/everforest))
- Rosé Pine and Rosé Pine Dawn ([rosepinetheme.com](https://rosepinetheme.com)). `oryx-hero` also uses the accent colors of Rosé Pine.
- Kanagawa ([rebelot/kanagawa.nvim](https://github.com/rebelot/kanagawa.nvim))
- Ayu Mirage and Light ([ayu-theme](https://github.com/ayu-theme/ayu-colors))
- Night Owl by Sarah Drasner ([sdras/night-owl-vscode-theme](https://github.com/sdras/night-owl-vscode-theme))
- Horizon ([jolaleye/horizon-theme-vscode](https://github.com/jolaleye/horizon-theme-vscode))
- Flexoki dark and light by Steph Ango ([stephango.com/flexoki](https://stephango.com/flexoki))
- GitHub Light ([primer/primitives](https://github.com/primer/primitives))

</details>

<details>
<summary><b>Bundled grammars</b> (beyond syntect's defaults, with thanks to their authors)</summary>

- TOML ([sublimehq/Packages](https://github.com/sublimehq/Packages))
- INI ([jwortmann/ini-syntax](https://github.com/jwortmann/ini-syntax), MIT)
- Kotlin ([guille/sublime-kotlin](https://github.com/guille/sublime-kotlin), public domain)
- Swift ([aerobounce/Swift-Next](https://github.com/aerobounce/Swift-Next), MIT)
- TypeScript and TSX, Microsoft's grammars (Apache-2.0) as converted by [bat](https://github.com/sharkdp/bat)
- Dockerfile ([keith-hall/Containerfile-sublime-syntax](https://github.com/keith-hall/Containerfile-sublime-syntax), MIT)
- Zig ([ziglang/sublime-zig-language](https://github.com/ziglang/sublime-zig-language), MIT)
- Terraform and HCL ([alexlouden/Terraform.tmLanguage](https://github.com/alexlouden/Terraform.tmLanguage), MIT)
- GraphQL ([dncrews/GraphQL-SublimeText3](https://github.com/dncrews/GraphQL-SublimeText3), MIT)
- Protocol Buffers ([VcamX/protobuf-syntax-highlighting](https://github.com/VcamX/protobuf-syntax-highlighting), MIT)

Each grammar ships with its license text beside the source under `assets/syntaxes/`.

</details>

<br>

<div align="center">

Oryx is free software, released under the [GNU General Public License v3.0](LICENSE).

</div>
