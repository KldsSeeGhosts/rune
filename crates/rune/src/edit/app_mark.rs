//! App-owned, nonvisual inline marks.

use super::*;

/// An app-defined mark. Its id has no framework meaning and it supplies no text style.
/// By default insertion at either boundary stays outside the mark.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct AppMark {
    id: String,
    inclusive_start: bool,
    inclusive_end: bool,
}

impl AppMark {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into(), inclusive_start: false, inclusive_end: false }
    }
    pub fn inclusive(mut self, start: bool, end: bool) -> Self {
        self.inclusive_start = start;
        self.inclusive_end = end;
        self
    }
    pub fn id(&self) -> &str { &self.id }
    pub fn inclusive_start(&self) -> bool { self.inclusive_start }
    pub fn inclusive_end(&self) -> bool { self.inclusive_end }

    pub(super) fn map(&self, range: Range<usize>, a: usize, b: usize, inserted: usize) -> Range<usize> {
        let mut map = EditMap::default();
        map.push(a..b, inserted);
        map.map_range(range,
            if self.inclusive_start { MapBias::Before } else { MapBias::After },
            if self.inclusive_end { MapBias::After } else { MapBias::Before })
    }
}

impl TextEdit {
    /// Set an app mark on a nonempty, char-boundary-clamped byte range as one undo step.
    /// App marks are allowed in code and survive formatting allowlists.
    pub fn set_app_mark(&mut self, range: Range<usize>, mark: AppMark) {
        if self.secure { return; }
        self.sync_invariants();
        let a = Self::clamp_char_boundary(&self.text, range.start);
        let b = Self::clamp_char_boundary(&self.text, range.end);
        if a >= b { return; }
        self.break_history_group();
        let before = self.snapshot();
        self.marks.push(MarkRange::new(a..b, Mark::App(mark)));
        self.normalize_marks();
        self.record_change(before, EditKind::Other);
    }

    pub fn app_marks(&self) -> impl Iterator<Item = (Range<usize>, &AppMark)> {
        self.marks.iter().filter_map(|m| match &m.mark {
            Mark::App(mark) => Some((m.range(), mark)), _ => None,
        })
    }

    /// Remove every range carrying this app id, as one undo step.
    pub fn remove_app_mark(&mut self, id: &str) {
        if self.secure { return; }
        self.sync_invariants();
        self.break_history_group();
        let before = self.snapshot();
        self.marks.retain(|m| !matches!(&m.mark, Mark::App(a) if a.id() == id));
        self.record_change(before, EditKind::Other);
    }
}
