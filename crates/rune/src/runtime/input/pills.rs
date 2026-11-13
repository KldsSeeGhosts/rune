//! Additional pill activation in one editable input, independent of caret selection.
use super::*;
use std::ops::Range;

/// The occurrence under a held input press. Logical identity survives ending a typing run,
/// unlike the editor's copy-on-write history allocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    id: String,
    range: Range<usize>,
    block: crate::BlockId,
    ordinal: usize,
    count: usize,
}

impl Target {
    fn same_occurrence(&self, other: &Self) -> bool {
        self.id == other.id && self.block == other.block && self.ordinal == other.ordinal && self.count == other.count
    }

    fn current_range(&self, edit: &crate::TextEdit) -> Option<Range<usize>> {
        targets(edit).into_iter().find(|t| self.same_occurrence(t)).map(|t| t.range)
    }
}

fn targets(edit: &crate::TextEdit) -> Vec<Target> {
    if edit.is_secure() { return Vec::new(); }
    let mut counts = HashMap::<crate::BlockId, usize>::default();
    let mut targets: Vec<_> = edit.pills().filter_map(|(range, id)| {
        let block = edit.logical_id_at(range.start)?;
        let ordinal = counts.entry(block).or_default();
        let target = Target { id: id.to_owned(), range, block, ordinal: *ordinal, count: 0 };
        *ordinal += 1;
        Some(target)
    }).collect();
    for t in &mut targets { t.count = counts[&t.block]; }
    targets
}

/// Rectangles in `rect`'s coordinate space, at `zoom` points per layout point. Reuse the
/// painter's chip geometry, including wrapped fragments, padding, icons and RTL placement.
fn boxes(n: &Node, rect: [f32; 4], scale: f32, zoom: f32, scroll: f32) -> Vec<(Target, [f32; 4])> {
    let Some(input) = n.input.as_ref().filter(|i| !i.edit.is_secure()) else { return Vec::new() };
    let (Some(spec), Some(layout)) = (&n.text, &n.text_layout) else { return Vec::new() };
    if spec.redacted || spec.cold.pills.is_empty() || n.elided.is_some() { return Vec::new(); }
    let mut origin = links::origin(n, spec, layout, rect, scale, zoom);
    if input.single_line { origin[0] -= scroll * zoom; }
    let px = scale / zoom.max(1e-4);
    let painted = crate::text::pill_rects(layout, &spec.cold.pills, spec.style.size, scale, origin.map(|v| v * px));
    let mut out = Vec::new();
    for target in targets(&input.edit) {
        let mapped = input.edit.map_range_to_display(target.range.start, target.range.end);
        let [(start, end)] = mapped[..] else { continue };
        let Some(k) = spec.cold.pills.iter().position(|p| p.start == start && p.end == end) else { continue };
        for (_, r) in painted.iter().filter(|(i, _)| *i == k) {
            out.push((target.clone(), r.map(|v| v / px)));
        }
    }
    out
}

const RECT: &str = "\u{1}input-pill:";

fn rect_key(input: &str, id: &str, range: Option<Range<usize>>) -> String {
    let which = range.map_or_else(|| "*".into(), |r| format!("{}-{}", r.start, r.end));
    format!("{RECT}{which}:{}:{input}{id}", input.len())
}

fn query(key: &str) -> Option<(&str, &str, Option<Range<usize>>)> {
    let (range, rest) = key.strip_prefix(RECT)?.split_once(':')?;
    let range = if range == "*" { None } else {
        let (a, b) = range.split_once('-')?;
        let r = a.parse::<usize>().ok()?..b.parse::<usize>().ok()?;
        if r.start >= r.end { return None; }
        Some(r)
    };
    let (len, rest) = rest.split_once(':')?;
    let len = len.parse::<usize>().ok()?;
    Some((rest.get(..len)?, rest.get(len..)?, range))
}

impl<S> Cx<S> {
    /// The first pill with `id` in input `key`, as `[x, y, w, h]` in window layout points.
    /// Wrapped fragments are united. Includes chip padding and icons, but not outside
    /// margins. Missing, concealed or unmounted pills return None.
    ///
    /// Like [`Self::window_layout`], this read tracks scrolling and rewrapping, is unclipped,
    /// and excludes visual transforms and animation. Use [`crate::anchor_rect`] and a
    /// root-level pinned overlay to anchor a popover. For duplicate ids, use
    /// [`Self::input_pill_range_rect`] with the range from [`El::on_input_pill_press`].
    #[track_caller]
    pub fn input_pill_rect(&self, key: &str, id: &str) -> Option<[f32; 4]> {
        self.window_layout(&rect_key(key, id, None))
    }

    /// [`Self::input_pill_rect`] for the exact model byte range of one occurrence.
    #[track_caller]
    pub fn input_pill_range_rect(&self, key: &str, id: &str, range: Range<usize>) -> Option<[f32; 4]> {
        self.window_layout(&rect_key(key, id, Some(range)))
    }
}

pub(super) fn is_rect_key(key: &str) -> bool { key.starts_with(RECT) }

impl<S: 'static> Runtime<S> {
    fn pill_scroll(&self, n: &Node) -> f32 {
        let Some(input) = n.input.as_ref().filter(|i| i.single_line && i.edit.focused) else { return 0.0 };
        let (Some(spec), Some(layout)) = (&n.text, &n.text_layout) else { return 0.0 };
        let s = self.env.scale;
        single_line_offset(self.input_scroll.get(&n.path).copied().unwrap_or(0.0), input,
            &spec.content, layout, s, (n.local.w - n.inset[0] - n.inset[2]).max(0.0) * s) / s
    }

    pub(super) fn input_pill_layout_rect(
        &self, nodes: &[Node], index: Option<&key_index::KeyIndex>, key: &str, scrolled: bool,
    ) -> Option<[f32; 4]> {
        let (input, id, range) = query(key)?;
        let i = index.map_or_else(|| key_index::scan_first(nodes, input), |index| index.first(nodes, input))?;
        if nodes.as_ptr() == self.nodes.as_ptr() && self.subtree_exiting(i) { return None; }
        let r = self.placed(nodes, i, scrolled);
        let boxes = boxes(&nodes[i], [r[0], r[1], r[0] + r[2], r[1] + r[3]], self.env.scale, 1.0, self.pill_scroll(&nodes[i]));
        let first = boxes.iter().find(|(t, _)| t.id == id && range.as_ref().is_none_or(|r| *r == t.range))?.0.clone();
        let r = boxes.iter().filter(|(t, _)| *t == first).map(|(_, r)| *r)
            .reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), (a[0] + a[2]).max(b[0] + b[2]) - a[0].min(b[0]), (a[1] + a[3]).max(b[1] + b[3]) - a[1].min(b[1])])?;
        Some(r)
    }

    fn input_pill_at(&self, i: usize, x: f32, y: f32) -> Option<Target> {
        let n = &self.nodes[i];
        let (rect, x, y) = self.unturned_box_and_point(i, x, y);
        let zoom = self.drawn_scale.get(i).copied().unwrap_or(1.0);
        boxes(n, rect, self.env.scale, zoom, self.pill_scroll(n)).into_iter().find_map(|(t, r)| {
            contains([r[0], r[1], r[0] + r[2], r[1] + r[3]], x, y).then_some(t)
        })
    }

    pub(super) fn input_pill_down(&self, i: usize, clicks: u8, x: f32, y: f32) -> Option<Target> {
        if clicks != 1 || self.nodes[i].input.as_ref()?.pill_press.is_none() { return None; }
        self.input_pill_at(i, x, y)
    }

    pub(super) fn input_pill_up(&mut self, held: &capture::Held, x: f32, y: f32) {
        let Some(target) = held.input_pill.as_ref().filter(|_| held.inside() && !held.moved && !held.left && held.button == crate::PointerButton::Primary) else { return };
        let Some(i) = self.live_index(&held.path).filter(|&i| self.allowed(i)) else { return };
        if self.primary_target(x, y).and_then(|j| self.textbox_input(j)) != Some(i) { return; }
        if let Some(released) = self.input_pill_at(i, x, y).filter(|t| target.same_occurrence(t)) {
            self.queue.push(Action::InputPill(held.path.clone(), released));
        }
    }

    pub(super) fn press_input_pill(&mut self, path: &str, target: Target, tx: &mut Tx) -> bool {
        let Some((access, callback)) = self.node(path).and_then(|n| {
            let i = n.input.as_ref()?;
            Some((i.access.clone()?, i.pill_press.clone()?))
        }) else { return false };
        if self.focus.path() != Some(path) { return false; }
        let Some(range) = target.current_range(access(states!(self))) else { return false };
        callback(states!(self), &target.id, range, tx);
        true
    }

    fn selected_input_pill(&mut self, path: &str) -> Option<Target> {
        if self.focus.path() != Some(path) { return None; }
        let access = self.focused_input().and_then(|i| i.pill_press.as_ref().and(i.access.clone()))?;
        let edit = access(states!(self));
        if edit.is_secure() || edit.is_composing() { return None; }
        let (a, b) = edit.range();
        targets(edit).into_iter().find(|t| t.range == (a..b))
    }

    pub(super) fn input_pill_key(&mut self, ev: &KeyEvent, repeat: bool, tx: &mut Tx) -> bool {
        if ev.mods != crate::Mods::default()
            || !(ev.key == crate::Key::Enter || matches!(&ev.key, crate::Key::Text(t) if t == " ")) { return false; }
        let candidate = self.focused_input().is_some_and(|i| {
            i.pill_press.is_some() && i.access.is_some() && !i.edit.is_secure() && !i.edit.is_composing()
                && i.edit.pills().any(|(r, _)| (r.start, r.end) == i.edit.range())
        });
        if !candidate { return false; }
        // A declining key handler can unmount the field. Resolve a current binding before
        // reading app state, just like the queued pointer activation's route boundary.
        self.commit_for(tx, CommitWhy::ROUTE);
        let Some(path) = self.focus.owned() else { return false };
        let Some(target) = self.selected_input_pill(&path) else { return false };
        if !self.playground_typing() && let Some(f) = self.on_key {
            if Tx::took(self.app_key(f, ev, tx), tx) { return true; }
            self.commit_for(tx, CommitWhy::ROUTE);
        }
        if !repeat && let Some(current) = self.selected_input_pill(&path).filter(|t| target.same_occurrence(t)) {
            self.press_input_pill(&path, current, tx);
        }
        true
    }
}

#[cfg(test)]
#[path = "pills_tests.rs"]
mod tests;
