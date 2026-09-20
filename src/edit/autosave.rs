//! Autosave: the rule that decides whether an automatic save may
//! write, and the pause after the last edit that one of the two
//! settings waits for. Pure, so both are testable without a window;
//! `App` owns the wiring.

use std::time::{Duration, Instant, SystemTime};

/// The longest pause the settings row offers, in seconds.
pub const PAUSE_MAX: u32 = 60;

/// What an automatic save does at its moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing is unsaved, or the text has no file of its own to go to.
    Nothing,
    /// The file on disk is no longer the one Oryx read or wrote: the
    /// write waits for the reader's own Ctrl+S.
    Hold,
    Write,
}

/// What the rule reads. `seen` is the file's identity (modified time
/// and length) recorded at the last read or write, `now` what the disk
/// answers at this moment.
#[derive(Debug, Clone, Copy)]
pub struct Facts {
    pub unsaved: bool,
    pub note: bool,
    pub deleted: bool,
    /// The reader was already told the file changed on disk.
    pub conflict: bool,
    pub seen: Option<(SystemTime, u64)>,
    pub now: Option<(SystemTime, u64)>,
}

/// An automatic save never overwrites someone else's change: it writes
/// only while the file on disk is provably the one last read or
/// written. Ctrl+S is the reader's word and is not bound by this.
pub fn verdict(facts: Facts) -> Verdict {
    if !facts.unsaved || facts.note {
        return Verdict::Nothing;
    }
    let untouched = facts.seen.is_some() && facts.seen == facts.now;
    if facts.deleted || facts.conflict || !untouched {
        return Verdict::Hold;
    }
    Verdict::Write
}

/// The pause save's deadline. Every edit moves it, so a burst of typing
/// costs one write, and it is gone once taken, so an idle window has
/// nothing to wake for.
#[derive(Debug, Default)]
pub struct Pause {
    at: Option<Instant>,
}

impl Pause {
    /// An edit landed at `now`; `seconds` is the setting, 0 for off.
    pub fn edited(&mut self, now: Instant, seconds: u32) {
        self.at = (seconds > 0).then(|| now + Duration::from_secs(u64::from(seconds)));
    }

    /// When the loop should wake for the save, if at all.
    pub fn wake(&self) -> Option<Instant> {
        self.at
    }

    /// True once, when the pause has lasted: the deadline is taken.
    pub fn take_due(&mut self, now: Instant) -> bool {
        let due = self.at.is_some_and(|at| now >= at);
        if due {
            self.at = None;
        }
        due
    }

    pub fn clear(&mut self) {
        self.at = None;
    }
}

/// The settings row's value: "off" at 0, else the seconds in words.
pub fn pause_label(seconds: u32) -> String {
    match seconds {
        0 => "off".to_string(),
        1 => "1 second".to_string(),
        n => format!("{n} seconds"),
    }
}

/// One step of the settings row, held to `0..=PAUSE_MAX`.
pub fn step_pause(seconds: u32, delta: i32) -> u32 {
    seconds.saturating_add_signed(delta).min(PAUSE_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(secs: u64, len: u64) -> Option<(SystemTime, u64)> {
        Some((SystemTime::UNIX_EPOCH + Duration::from_secs(secs), len))
    }

    fn clean_edit() -> Facts {
        Facts {
            unsaved: true,
            note: false,
            deleted: false,
            conflict: false,
            seen: state(100, 12),
            now: state(100, 12),
        }
    }

    #[test]
    fn unsaved_edits_over_an_untouched_file_are_written() {
        assert_eq!(verdict(clean_edit()), Verdict::Write);
    }

    #[test]
    fn nothing_unsaved_writes_nothing() {
        let facts = Facts {
            unsaved: false,
            ..clean_edit()
        };
        assert_eq!(verdict(facts), Verdict::Nothing);
    }

    #[test]
    fn the_note_is_left_alone() {
        let facts = Facts {
            note: true,
            ..clean_edit()
        };
        assert_eq!(verdict(facts), Verdict::Nothing);
    }

    #[test]
    fn a_file_changed_on_disk_holds_the_write() {
        let newer = Facts {
            now: state(160, 12),
            ..clean_edit()
        };
        assert_eq!(verdict(newer), Verdict::Hold, "another modified time");
        let longer = Facts {
            now: state(100, 40),
            ..clean_edit()
        };
        assert_eq!(verdict(longer), Verdict::Hold, "another length");
    }

    #[test]
    fn a_change_the_reader_was_told_about_keeps_holding() {
        // The disk check records the new identity when it tells the
        // reader, so the two states agree again; the flag keeps the hold
        // until Ctrl+S or a reload.
        let facts = Facts {
            conflict: true,
            seen: state(160, 12),
            now: state(160, 12),
            ..clean_edit()
        };
        assert_eq!(verdict(facts), Verdict::Hold);
    }

    #[test]
    fn a_file_gone_from_disk_is_not_written_back() {
        let missing = Facts {
            now: None,
            ..clean_edit()
        };
        assert_eq!(verdict(missing), Verdict::Hold, "missing at this moment");
        let declared = Facts {
            deleted: true,
            now: None,
            ..clean_edit()
        };
        assert_eq!(verdict(declared), Verdict::Hold, "declared deleted");
    }

    #[test]
    fn an_identity_never_recorded_holds_the_write() {
        let facts = Facts {
            seen: None,
            ..clean_edit()
        };
        assert_eq!(verdict(facts), Verdict::Hold);
    }

    #[test]
    fn an_edit_arms_the_pause_and_the_setting_at_zero_does_not() {
        let t0 = Instant::now();
        let mut pause = Pause::default();
        assert_eq!(pause.wake(), None, "nothing to wake for before an edit");
        pause.edited(t0, 0);
        assert_eq!(pause.wake(), None, "off");
        pause.edited(t0, 5);
        assert_eq!(pause.wake(), Some(t0 + Duration::from_secs(5)));
    }

    #[test]
    fn a_burst_of_typing_costs_one_write() {
        let t0 = Instant::now();
        let mut pause = Pause::default();
        let mut writes = 0;
        // A key every 200 ms for four seconds, the loop waking between.
        for tick in 0..20 {
            let now = t0 + Duration::from_millis(200 * tick);
            if pause.take_due(now) {
                writes += 1;
            }
            pause.edited(now, 2);
        }
        assert_eq!(writes, 0, "no write mid-burst");
        let last = t0 + Duration::from_millis(200 * 19);
        assert!(!pause.take_due(last + Duration::from_millis(1999)));
        assert!(pause.take_due(last + Duration::from_secs(2)));
        assert_eq!(pause.wake(), None, "nothing wakes the loop after the write");
        assert!(!pause.take_due(last + Duration::from_secs(60)));
    }

    #[test]
    fn turning_the_setting_off_drops_a_standing_deadline() {
        let t0 = Instant::now();
        let mut pause = Pause::default();
        pause.edited(t0, 5);
        pause.edited(t0 + Duration::from_secs(1), 0);
        assert_eq!(pause.wake(), None);
    }

    #[test]
    fn the_row_steps_by_a_second_between_off_and_a_minute() {
        assert_eq!(step_pause(0, 1), 1);
        assert_eq!(step_pause(0, -1), 0);
        assert_eq!(step_pause(59, 1), PAUSE_MAX);
        assert_eq!(step_pause(PAUSE_MAX, 1), PAUSE_MAX);
        assert_eq!(step_pause(3, -1), 2);
    }

    #[test]
    fn the_row_reads_off_or_the_seconds() {
        assert_eq!(pause_label(0), "off");
        assert_eq!(pause_label(1), "1 second");
        assert_eq!(pause_label(5), "5 seconds");
    }
}
