//! The workspace's file list: one walk of the Files root through the
//! `ignore` crate, which supplies the rules fd and ripgrep apply —
//! `.gitignore`, `.ignore`, hidden files, symlinks left alone — and an
//! efficient traversal. Only paths are kept; nothing reads a file's
//! contents, so a large tree indexes cheaply and the walk stays the
//! sole filesystem cost.

use std::path::{Path, PathBuf};

use super::types::RootGeneration;

/// One indexed file: its path below the root, `/`-separated whatever
/// the platform's separator, with the byte offset where the file's own
/// name starts. The absolute path is the root joined with this, so it
/// is never stored twice.
#[derive(Debug)]
pub struct IndexedFile {
    pub relative_path: Box<str>,
    pub basename_start: usize,
}

/// The files under one root, as of one walk.
#[derive(Debug)]
pub struct WorkspaceIndex {
    root: PathBuf,
    generation: RootGeneration,
    files: Vec<IndexedFile>,
}

/// Why a walk did not produce an index. `Superseded` is not a failure
/// of the root: a newer generation took over and the caller moves on.
#[derive(Debug)]
pub enum ScanFailure {
    /// The root cannot be read at all: missing, or not ours to list.
    Unreadable(String),
    /// The walk was abandoned; a newer generation stands.
    Superseded,
}

impl WorkspaceIndex {
    /// How many files the index holds.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The root the files were walked from.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The generation the index was built under.
    pub fn generation(&self) -> RootGeneration {
        self.generation
    }

    /// The indexed files, in lexical path order.
    pub fn files(&self) -> &[IndexedFile] {
        &self.files
    }

    /// The absolute path an indexed file lives at.
    pub fn absolute(&self, file: &IndexedFile) -> PathBuf {
        // The stored separators are `/`; Windows accepts them, and the
        // join below is what every caller opens through.
        self.root.join(self.root_sep(&file.relative_path))
    }

    /// The relative path in the platform's own separators, so joined
    /// results compare equal with the sidebar's native-separator paths.
    fn root_sep(&self, relative: &str) -> String {
        relative.replace('/', std::path::MAIN_SEPARATOR_STR)
    }

    /// Walks `root` into an index, checking `superseded` as it goes so
    /// a root the UI has left behind stops costing IO. Entries that
    /// cannot be named in UTF-8 are skipped, as the sidebar's tree
    /// already skips them; files a walk cannot read are skipped too
    /// rather than failing the whole index.
    pub fn scan(
        root: &Path,
        generation: RootGeneration,
        superseded: &dyn Fn() -> bool,
    ) -> Result<WorkspaceIndex, ScanFailure> {
        let walked = std::fs::metadata(root)
            .map_err(|e| ScanFailure::Unreadable(format!("{}: {e}", root.display())))?;
        if !walked.is_dir() {
            return Err(ScanFailure::Unreadable(format!(
                "{}: not a folder",
                root.display()
            )));
        }
        let walker = ignore::WalkBuilder::new(root)
            .follow_links(false)
            // A `.gitignore` in a folder no repository claims still
            // hides its files, the way the rules read everywhere
            // outside a checkout.
            .require_git(false)
            .build();
        let mut files: Vec<IndexedFile> = Vec::new();
        for entry in walker {
            if files.len() % 1024 == 0 && superseded() {
                return Err(ScanFailure::Superseded);
            }
            let Ok(entry) = entry else {
                continue;
            };
            // Directories are not results; with `follow_links` off, a
            // symlink lists as neither file nor dir and stays out.
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let Some(relative) = entry.path().strip_prefix(root).ok() else {
                continue;
            };
            // The walk reports platform separators; matching, ordering
            // and display all want `/`.
            let Some(name) = relative.to_str() else {
                continue;
            };
            let relative = name.replace(std::path::MAIN_SEPARATOR, "/");
            let basename_start = relative.rfind('/').map_or(0, |at| at + 1);
            files.push(IndexedFile {
                relative_path: relative.into_boxed_str(),
                basename_start,
            });
        }
        files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(WorkspaceIndex {
            root: root.to_path_buf(),
            generation,
            files,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oryx-search-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn walked(index: &WorkspaceIndex) -> Vec<String> {
        index
            .files()
            .iter()
            .map(|f| f.relative_path.to_string())
            .collect()
    }

    const NEVER: fn() -> bool = || false;

    #[test]
    fn a_walk_reaches_every_subfolder_and_keeps_lexical_order() {
        let dir = fresh("recurse");
        std::fs::create_dir_all(dir.join("src/ui")).unwrap();
        for f in [
            "README.md",
            "src/main.rs",
            "src/ui/mod.rs",
            "src/ui/sidebar.rs",
            "zeta.txt",
        ] {
            std::fs::write(dir.join(f), "x").unwrap();
        }
        let index = WorkspaceIndex::scan(&dir, RootGeneration(1), &NEVER).unwrap();
        assert_eq!(
            walked(&index),
            [
                "README.md",
                "src/main.rs",
                "src/ui/mod.rs",
                "src/ui/sidebar.rs",
                "zeta.txt"
            ]
        );
        assert_eq!(index.len(), 5);
        assert_eq!(index.root(), dir.as_path());
        assert_eq!(index.generation(), RootGeneration(1));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn basename_starts_after_the_last_separator() {
        let dir = fresh("basename");
        std::fs::create_dir_all(dir.join("src/ui")).unwrap();
        std::fs::write(dir.join("src/ui/sidebar.rs"), "x").unwrap();
        std::fs::write(dir.join("top.md"), "x").unwrap();
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        let deep = index
            .files()
            .iter()
            .find(|f| &*f.relative_path == "src/ui/sidebar.rs")
            .unwrap();
        assert_eq!(&deep.relative_path[deep.basename_start..], "sidebar.rs");
        let top = index
            .files()
            .iter()
            .find(|f| &*f.relative_path == "top.md")
            .unwrap();
        assert_eq!(
            top.basename_start, 0,
            "no separator before a top-level name"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn gitignore_and_ignore_rules_keep_files_out() {
        let dir = fresh("gitignore");
        std::fs::create_dir_all(dir.join("target/debug")).unwrap();
        std::fs::create_dir_all(dir.join("node_modules/pkg")).unwrap();
        std::fs::write(dir.join("kept.rs"), "x").unwrap();
        std::fs::write(dir.join("target/debug/skipped.rs"), "x").unwrap();
        std::fs::write(dir.join("node_modules/pkg/skipped.js"), "x").unwrap();
        std::fs::write(dir.join(".gitignore"), "target/\nnode_modules/\n").unwrap();
        std::fs::write(dir.join("also-kept.rs"), "x").unwrap();
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        // The `.gitignore` itself is hidden, so it stays out too.
        assert_eq!(walked(&index), ["also-kept.rs", "kept.rs"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_dot_ignore_file_hides_its_files_too() {
        let dir = fresh("dot-ignore");
        std::fs::write(dir.join("kept.md"), "x").unwrap();
        std::fs::write(dir.join("hidden.md"), "x").unwrap();
        std::fs::write(dir.join(".ignore"), "hidden.md\n").unwrap();
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        assert_eq!(walked(&index), ["kept.md"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn hidden_files_and_folders_stay_out() {
        let dir = fresh("hidden");
        std::fs::create_dir_all(dir.join(".git/objects")).unwrap();
        std::fs::write(dir.join(".git/config"), "x").unwrap();
        std::fs::write(dir.join(".env"), "x").unwrap();
        std::fs::write(dir.join("visible.md"), "x").unwrap();
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        assert_eq!(walked(&index), ["visible.md"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A directory symlink is listed by no walk that refuses to follow
    /// it, and neither are the files inside it.
    #[cfg(unix)]
    #[test]
    fn a_directory_symlink_is_not_followed() {
        let dir = fresh("symlink");
        std::fs::create_dir_all(dir.join("real")).unwrap();
        std::fs::write(dir.join("real/inner.md"), "x").unwrap();
        std::fs::write(dir.join("outer.md"), "x").unwrap();
        std::os::unix::fs::symlink(dir.join("real"), dir.join("linked")).unwrap();
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        assert_eq!(
            walked(&index),
            ["outer.md", "real/inner.md"],
            "the link itself is not a file, and its target is not walked"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_root_reports_unreadable() {
        let missing = fresh("gone");
        std::fs::remove_dir_all(&missing).unwrap();
        let failure = WorkspaceIndex::scan(&missing, RootGeneration(0), &NEVER).unwrap_err();
        assert!(matches!(failure, ScanFailure::Unreadable(_)));
    }

    #[test]
    fn a_file_as_root_reports_unreadable() {
        let dir = fresh("file-root");
        let file = dir.join("plain.md");
        std::fs::write(&file, "x").unwrap();
        let failure = WorkspaceIndex::scan(&file, RootGeneration(0), &NEVER).unwrap_err();
        assert!(matches!(failure, ScanFailure::Unreadable(_)));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A walk abandoned mid-course reports `Superseded` rather than an
    /// index that pretends to be complete.
    #[test]
    fn a_superseded_walk_stops() {
        let dir = fresh("superseded");
        for n in 0..2048 {
            std::fs::write(dir.join(format!("f{n:04}.md")), "x").unwrap();
        }
        let now = std::sync::atomic::AtomicUsize::new(0);
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &|| {
            now.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 0
        })
        .unwrap_err();
        assert!(matches!(index, ScanFailure::Superseded));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// An unreadable folder inside the tree is skipped, not fatal.
    #[cfg(unix)]
    #[test]
    fn an_unreadable_folder_is_skipped_not_fatal() {
        use std::os::unix::fs::PermissionsExt;
        let dir = fresh("permission");
        std::fs::create_dir_all(dir.join("sealed")).unwrap();
        std::fs::write(dir.join("kept.md"), "x").unwrap();
        std::fs::write(dir.join("sealed/kept-out.md"), "x").unwrap();
        let mut sealed = std::fs::metadata(dir.join("sealed")).unwrap().permissions();
        sealed.set_mode(0o000);
        std::fs::set_permissions(dir.join("sealed"), sealed).unwrap();
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        assert_eq!(walked(&index), ["kept.md"]);
        let mut open = std::fs::metadata(dir.join("sealed")).unwrap().permissions();
        open.set_mode(0o755);
        std::fs::set_permissions(dir.join("sealed"), open).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_absolute_path_joins_the_root_back_on() {
        let dir = fresh("absolute");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/file.md"), "x").unwrap();
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        let file = &index.files()[0];
        assert!(index
            .absolute(file)
            .ends_with(Path::new("sub").join("file.md")));
        assert!(index.absolute(file).starts_with(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_empty_root_yields_an_empty_index() {
        let dir = fresh("empty");
        let index = WorkspaceIndex::scan(&dir, RootGeneration(0), &NEVER).unwrap();
        assert!(index.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
