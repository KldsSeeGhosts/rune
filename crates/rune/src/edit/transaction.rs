//! Recorded transactions and detached source rewrites over canonical snapshot history.

use super::*;
use super::history::Snapshot;

mod capture;
mod rewrite;
pub(super) use capture::State as Capture;
pub(super) use rewrite::State as Rewrite;

/// A recorded transaction becomes one undo step. A transient transaction keeps both history
/// stacks intact and holds their canonical document until the next history or recorded edit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransactionOptions {
    #[default]
    Recorded,
    Transient,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransactionError { Composing, Secure, Nested }

impl std::fmt::Display for TransactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{self:?} editor transaction") }
}
impl std::error::Error for TransactionError {}

impl TextEdit {
    /// Apply model operations and return their edit map. Use `replace_range`, `insert` and
    /// the attributed setters, not direct assignment to public fields.
    ///
    /// A transient rewrite maps the incoming selection unless the closure explicitly calls
    /// `set_selection` or `set_caret`. It does not clear redo. Undo/redo first restore the
    /// canonical document; the next recorded edit includes the rewrite in its own undo step.
    /// This rebases incoming edits rather than modifying old history snapshots in place.
    /// Rewrites refuse active IME composition and cannot nest in transactions or undo groups.
    /// Entering secure mode during the closure aborts capture and discards old history.
    /// That security change is retained even if the closure panics.
    pub fn transact(&mut self, options: TransactionOptions, f: impl FnOnce(&mut Self)) -> Result<EditMap, TransactionError> {
        if self.secure { return Err(TransactionError::Secure); }
        if self.is_composing() { return Err(TransactionError::Composing); }
        if self.group.is_open() || !matches!(self.capture, Capture::Idle) { return Err(TransactionError::Nested); }
        self.sync_invariants();
        let before = self.snapshot_now();
        let history = self.history.clone();
        let rewrite = self.rewrite.clone();
        let _ = capture::step(&mut self.capture, capture::Event::Begin);
        struct Reset<'a> {
            edit: &'a mut TextEdit,
            rollback: Option<(Snapshot, Arc<History>, Rewrite)>,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                let secured = matches!(self.edit.capture, Capture::Secured);
                let _ = capture::step(&mut self.edit.capture, capture::Event::Abort);
                self.edit.group = Group::Closed;
                if secured {
                    self.rollback = None;
                    self.edit.clear_history();
                    return;
                }
                if let Some((snapshot, history, rewrite)) = self.rollback.take() {
                    self.edit.restore(&snapshot);
                    self.edit.history = history;
                    let _ = rewrite::step(&mut self.edit.rewrite, rewrite::Event::Restore(rewrite));
                }
            }
        }
        let mut guard = Reset { edit: self, rollback: Some((before.clone(), history.clone(), rewrite)) };
        match options {
            TransactionOptions::Recorded => guard.edit.undo_group(f),
            TransactionOptions::Transient => {
                guard.edit.group = Group::Unrecorded;
                f(guard.edit);
                guard.edit.group = Group::Closed;
                if !matches!(guard.edit.capture, Capture::Secured) { guard.edit.history = history; }
            }
        }
        let effect = capture::step(&mut guard.edit.capture, capture::Event::Finish).into_iter().next();
        let (map, explicit_selection) = match effect {
            Some(capture::Effect::Finished { map, explicit_selection }) => (map, explicit_selection),
            Some(capture::Effect::Secured) => {
                guard.rollback = None;
                guard.edit.clear_history();
                return Err(TransactionError::Secure);
            }
            None => unreachable!(),
        };
        if options == TransactionOptions::Transient {
            if !explicit_selection {
                guard.edit.anchor = map.map(before.anchor, MapBias::After);
                guard.edit.focus = map.map(before.focus, MapBias::After);
            }
            let _ = rewrite::step(&mut guard.edit.rewrite, rewrite::Event::Rewritten { baseline: before, map: map.clone() });
        }
        guard.edit.sync_invariants();
        guard.rollback = None;
        Ok(map)
    }

    /// Replace a byte range without first moving the selection. Endpoints clamp to character
    /// boundaries, and a range cutting into a pill expands to cover it.
    pub fn replace_range(&mut self, range: Range<usize>, text: &str) {
        let a = Self::clamp_char_boundary(&self.text, range.start.min(range.end));
        let b = Self::clamp_char_boundary(&self.text, range.start.max(range.end));
        self.replace((a, b), text, EditKind::Other);
    }

    pub(super) fn capture_replace(&mut self, a: usize, b: usize, inserted: usize) {
        let _ = capture::step(&mut self.capture, capture::Event::Replace { range: a..b, inserted });
    }

    pub(super) fn explicit_selection(&mut self) {
        let _ = capture::step(&mut self.capture, capture::Event::Selection);
    }

    pub(super) fn invalidate_capture(&mut self) {
        let _ = capture::step(&mut self.capture, capture::Event::Secure);
    }

    pub(super) fn discard_rewrite(&mut self) {
        let _ = rewrite::step(&mut self.rewrite, rewrite::Event::Discard);
    }

    /// Canonical baseline for a new recorded edit, with its selection mapped back from the
    /// detached source. The recorded after-state includes that source plus the actual edit.
    pub(super) fn take_rewrite_baseline(&mut self, anchor: usize, focus: usize) -> Option<Snapshot> {
        rewrite::step(&mut self.rewrite, rewrite::Event::Consume { anchor, focus })
            .into_iter().next().map(|rewrite::Effect::Baseline(snapshot)| snapshot)
    }

    pub(super) fn restore_canonical(&mut self) {
        if let Some(snapshot) = self.take_rewrite_baseline(self.anchor, self.focus) { self.restore(&snapshot); }
    }
}
