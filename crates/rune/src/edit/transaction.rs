//! Recorded transactions and detached source rewrites over canonical snapshot history.

use super::*;
use super::history::Snapshot;

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

#[derive(Default)]
pub(super) enum Capture {
    #[default]
    Idle,
    Active { map: EditMap, explicit_selection: bool },
}

impl Clone for Capture {
    fn clone(&self) -> Self { Self::Idle }
}
impl PartialEq for Capture {
    fn eq(&self, _: &Self) -> bool { true }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub(super) enum Rewrite {
    #[default]
    Canonical,
    Detached { baseline: Snapshot, map: EditMap },
}

impl TextEdit {
    /// Apply model operations and return their edit map. Use `replace_range`, `insert` and
    /// the attributed setters, not direct assignment to public fields.
    ///
    /// A transient rewrite maps the incoming selection unless the closure explicitly calls
    /// `set_selection` or `set_caret`. It does not clear redo. Undo/redo first restore the
    /// canonical document; the next recorded edit includes the rewrite in its own undo step.
    /// This rebases incoming edits rather than modifying old history snapshots in place.
    /// Rewrites refuse active IME composition and cannot nest in transactions or undo groups.
    pub fn transact(&mut self, options: TransactionOptions, f: impl FnOnce(&mut Self)) -> Result<EditMap, TransactionError> {
        if self.secure { return Err(TransactionError::Secure); }
        if self.is_composing() { return Err(TransactionError::Composing); }
        if self.group.is_open() || !matches!(self.capture, Capture::Idle) { return Err(TransactionError::Nested); }
        self.sync_invariants();
        let before = self.snapshot_now();
        let history = self.history.clone();
        let rewrite = self.rewrite.clone();
        self.capture = Capture::Active { map: EditMap::default(), explicit_selection: false };
        struct Reset<'a> {
            edit: &'a mut TextEdit,
            rollback: Option<(Snapshot, Arc<History>, Rewrite)>,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.edit.capture = Capture::Idle;
                self.edit.group = Group::Closed;
                if let Some((snapshot, history, rewrite)) = self.rollback.take() {
                    self.edit.restore(&snapshot);
                    self.edit.history = history;
                    self.edit.rewrite = rewrite;
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
                guard.edit.history = history;
            }
        }
        let Capture::Active { map, explicit_selection } = std::mem::take(&mut guard.edit.capture) else { unreachable!() };
        if options == TransactionOptions::Transient {
            if !explicit_selection {
                guard.edit.anchor = map.map(before.anchor, MapBias::After);
                guard.edit.focus = map.map(before.focus, MapBias::After);
            }
            guard.edit.rewrite = match std::mem::take(&mut guard.edit.rewrite) {
                Rewrite::Canonical => Rewrite::Detached { baseline: before, map: map.clone() },
                Rewrite::Detached { baseline, map: previous } =>
                    Rewrite::Detached { baseline, map: previous.then(&map) },
            };
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
        if let Capture::Active { map, .. } = &mut self.capture { map.push(a..b, inserted); }
    }

    pub(super) fn explicit_selection(&mut self) {
        if let Capture::Active { explicit_selection, .. } = &mut self.capture { *explicit_selection = true; }
    }

    /// Canonical baseline for a new recorded edit, with its selection mapped back from the
    /// detached source. The recorded after-state includes that source plus the actual edit.
    pub(super) fn take_rewrite_baseline(&mut self, anchor: usize, focus: usize) -> Option<Snapshot> {
        let Rewrite::Detached { mut baseline, map } = std::mem::take(&mut self.rewrite) else { return None };
        let inverse = map.inverse();
        baseline.anchor = Self::clamp_char_boundary(&baseline.text, inverse.map(anchor, MapBias::After));
        baseline.focus = Self::clamp_char_boundary(&baseline.text, inverse.map(focus, MapBias::After));
        Some(baseline)
    }

    pub(super) fn restore_canonical(&mut self) {
        if let Some(snapshot) = self.take_rewrite_baseline(self.anchor, self.focus) { self.restore(&snapshot); }
    }
}
