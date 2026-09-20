//! The untitled note's files. Each Oryx keeps its note in a folder of
//! its own under the notes root, beside a lock file it holds locked for
//! as long as it runs; the system frees the lock however the process
//! ends. A folder whose lock is free therefore belongs to an Oryx that
//! is gone: with text in its note it is a leftover to offer back at a
//! launch, without text it is swept.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

/// The note's file name, the one the title and the save dialog show.
pub const NOTE_NAME: &str = "untitled.md";
const LOCK_NAME: &str = "lock";

/// Where the note folders live: the state folder, or the local data
/// folder on a platform without one. Never the cache, which
/// `--clear-cache` and system cleaners may empty.
pub fn root(state: Option<&Path>, data_local: &Path) -> PathBuf {
    state.unwrap_or(data_local).join("notes")
}

/// This Oryx's note folder, held for the life of the process.
#[derive(Debug)]
pub struct Seat {
    dir: PathBuf,
    lock: File,
}

impl Seat {
    /// A fresh folder under `root`, its lock taken.
    pub fn claim(root: &Path) -> io::Result<Seat> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        for _ in 0..8 {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
            let dir = root.join(format!("{}-{nanos}-{serial}", std::process::id()));
            std::fs::create_dir_all(&dir)?;
            let lock = File::options()
                .create(true)
                .truncate(false)
                .write(true)
                .open(dir.join(LOCK_NAME))?;
            // A launch sweeping at this very instant may hold the new
            // lock, or have removed the folder from under it; another
            // name settles both.
            if lock.try_lock().is_ok() && dir.join(LOCK_NAME).exists() {
                let dir = dir.canonicalize().unwrap_or(dir);
                return Ok(Seat { dir, lock });
            }
        }
        Err(io::Error::other("no free note folder"))
    }

    /// The note's file inside the folder.
    pub fn note(&self) -> PathBuf {
        self.dir.join(NOTE_NAME)
    }

    /// The clean way out: the folder goes, the lock with it.
    pub fn release(self) {
        remove(&self.dir, self.lock);
    }
}

/// A note left by an Oryx that ended without its save question. The
/// lock is held while the leftover is on offer, so a second launch at
/// the same moment does not offer it too.
#[derive(Debug)]
pub struct Leftover {
    dir: PathBuf,
    text: String,
    lock: File,
}

impl Leftover {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn folder(&self) -> &Path {
        &self.dir
    }

    /// The reader's Discard, or the end of a recovery: the folder goes.
    pub fn discard(self) {
        remove(&self.dir, self.lock);
    }
}

/// The leftovers under `root`, the newest first, each with its lock
/// taken. Folders of a running Oryx are passed over; folders without
/// text are swept on the way.
pub fn leftovers(root: &Path) -> Vec<Leftover> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<(std::time::SystemTime, Leftover)> = entries
        .flatten()
        .filter_map(|entry| {
            let leftover = adopt(&entry.path())?;
            let written = std::fs::metadata(leftover.dir.join(NOTE_NAME))
                .and_then(|meta| meta.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            Some((written, leftover))
        })
        .collect();
    found.sort_by_key(|(written, _)| std::cmp::Reverse(*written));
    found.into_iter().map(|(_, leftover)| leftover).collect()
}

/// One folder taken over by name, for the second window a recovery
/// starts. None when the folder is held, gone or without text.
pub fn adopt(folder: &Path) -> Option<Leftover> {
    let lock = File::options()
        .write(true)
        .open(folder.join(LOCK_NAME))
        .ok()?;
    // A held lock is a running Oryx. A lock that cannot be tried at all
    // (a file system without locks) reads the same way: nothing is
    // offered and nothing is swept.
    lock.try_lock().ok()?;
    let text = std::fs::read(folder.join(NOTE_NAME))
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    if text.trim().is_empty() {
        remove(folder, lock);
        return None;
    }
    Some(Leftover {
        dir: folder.to_path_buf(),
        text,
        lock,
    })
}

/// Removes a folder whose lock is held here. The note goes first, under
/// the lock, so a launch that takes the lock the instant it is freed
/// finds no text to offer; the lock closes before the folder goes,
/// since Windows keeps a folder while a file in it is open.
fn remove(dir: &Path, lock: File) {
    std::fs::remove_file(dir.join(NOTE_NAME)).ok();
    drop(lock);
    std::fs::remove_dir_all(dir).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oryx-notes-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    /// A seat with `text` in its note, then the process "ends": the
    /// handle closes, the lock goes, the files stay, as after a crash.
    fn crashed(root: &Path, text: &str) -> PathBuf {
        let seat = Seat::claim(root).expect("a seat");
        std::fs::write(seat.note(), text).unwrap();
        let dir = seat.dir.clone();
        drop(seat);
        dir
    }

    #[test]
    fn the_root_is_the_state_folder_or_the_local_data_folder() {
        let (state, data) = (Path::new("/s/oryx"), Path::new("/d/oryx"));
        assert_eq!(root(Some(state), data), Path::new("/s/oryx/notes"));
        assert_eq!(root(None, data), Path::new("/d/oryx/notes"));
    }

    #[test]
    fn two_claims_get_two_folders_and_the_visible_name() {
        let root = scratch("claims");
        let (a, b) = (Seat::claim(&root).unwrap(), Seat::claim(&root).unwrap());
        assert_ne!(a.note(), b.note());
        assert_eq!(a.note().file_name().unwrap(), "untitled.md");
        assert!(a.note().starts_with(&root));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_running_oryx_keeps_its_note_off_the_offer() {
        let root = scratch("running");
        let seat = Seat::claim(&root).unwrap();
        std::fs::write(seat.note(), "still typing").unwrap();
        assert!(leftovers(&root).is_empty());
        assert!(adopt(&seat.dir).is_none(), "held by its Oryx");
        assert_eq!(
            std::fs::read_to_string(seat.note()).unwrap(),
            "still typing"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_note_whose_oryx_is_gone_is_offered_with_its_text() {
        let root = scratch("gone");
        let dir = crashed(&root, "the lost paragraph\n");
        let found = leftovers(&root);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text(), "the lost paragraph\n");
        assert_eq!(found[0].folder(), dir);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_folder_without_text_is_swept() {
        let root = scratch("swept");
        let blank = crashed(&root, " \n");
        let seat = Seat::claim(&root).unwrap();
        let bare = seat.dir.clone();
        drop(seat);
        assert!(leftovers(&root).is_empty());
        assert!(!blank.exists(), "a note of spaces is no note");
        assert!(!bare.exists(), "a seat that never held a note");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_leftover_on_offer_is_held_against_a_second_launch() {
        let root = scratch("held");
        crashed(&root, "one");
        let first = leftovers(&root);
        assert_eq!(first.len(), 1);
        assert!(leftovers(&root).is_empty(), "the first launch holds it");
        drop(first);
        assert_eq!(leftovers(&root).len(), 1, "Not now: offered again");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_newest_leftover_comes_first() {
        let root = scratch("order");
        let old = crashed(&root, "old");
        let new = crashed(&root, "new");
        let day = std::time::Duration::from_secs(86_400);
        let stamp = std::time::SystemTime::now() - day;
        File::options()
            .write(true)
            .open(old.join(NOTE_NAME))
            .unwrap()
            .set_modified(stamp)
            .unwrap();
        let found = leftovers(&root);
        let folders: Vec<&Path> = found.iter().map(Leftover::folder).collect();
        assert_eq!(folders, [new.as_path(), old.as_path()]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn discard_and_release_remove_their_folders() {
        let root = scratch("remove");
        let dir = crashed(&root, "unwanted");
        leftovers(&root).pop().expect("the leftover").discard();
        assert!(!dir.exists());
        let seat = Seat::claim(&root).unwrap();
        let dir = seat.dir.clone();
        std::fs::write(seat.note(), "named and saved elsewhere").unwrap();
        seat.release();
        assert!(!dir.exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn adopt_takes_a_free_folder_by_name() {
        let root = scratch("adopt");
        let dir = crashed(&root, "for the second window");
        let taken = adopt(&dir).expect("free, with text");
        assert_eq!(taken.text(), "for the second window");
        assert!(adopt(&dir).is_none(), "held now");
        drop(taken);
        assert!(adopt(&root.join("missing")).is_none());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_missing_root_has_no_leftovers() {
        let root = scratch("none").join("never-made");
        assert!(leftovers(&root).is_empty());
    }
}
