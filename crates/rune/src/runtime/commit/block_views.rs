//! Composite inputs keep a measured text leaf and separately measured widget subtrees.
//! Runtime ancestry stays intact for paint, scrolling, transforms, events and accessibility.
use super::*;
use crate::el::InputBlockPosition;
use crate::text::{BlockSlot, SlotPosition};

/// Widgets are detached only from the solver's child list, not Rune's element tree. This
/// uses the same existing leaf/subtree interface in LayoutStore and BaselineLayoutStore.
pub(super) fn detached(nodes: &[Node], parent: usize, key: Option<&str>) -> bool {
    nodes[parent].input.as_ref().is_some_and(|i| i.owns_block_view(key))
}

pub(super) fn owner(nodes: &[Node], mut child: usize) -> Option<usize> {
    while let Some(parent) = nodes[child].parent {
        if detached(nodes, parent, nodes[child].key.as_deref()) { return Some(parent); }
        child = parent;
    }
    None
}

fn roots(nodes: &[Node]) -> Vec<(usize, usize)> {
    nodes.iter().enumerate().filter_map(|(child, n)| {
        let parent = n.parent?;
        detached(nodes, parent, n.key.as_deref()).then_some((parent, child))
    }).collect()
}

pub(super) fn measure(
    taffy: &mut LayoutEngine, root: SlotId, room: Size<AvailableSpace>,
    nodes: &mut [Node], text: &mut TextSystem, scale: f32,
) {
    let roots = roots(nodes);
    if roots.is_empty() { return; }
    // A leading widget can widen an intrinsically sized input, changing a header's width.
    // Repeat only until the measured table settles; no pass rebuilds or mutates app state.
    for _ in 0..roots.len() + 2 {
        let mut tables: HashMap<usize, Vec<BlockSlot>> = HashMap::default();
        for &(parent, child) in &roots {
            let input = nodes[parent].input.as_ref().unwrap();
            let view = input.block_views.iter().find(|v| nodes[child].key.as_ref() == Some(&v.key)).unwrap();
            let spec = nodes[parent].text.as_ref().unwrap();
            let start = view.start;
            let paragraph = spec.content[..start].bytes().filter(|&b| b == b'\n').count();
            let rtl = match spec.base() {
                parley::BaseDirection::Auto => crate::Direction::of(spec.content[start..].split('\n').next().unwrap_or("")).is_some_and(crate::Direction::is_rtl),
                parley::BaseDirection::Rtl => true,
                parley::BaseDirection::Ltr => false,
            };
            let l = taffy.box_layout(nodes[parent].taffy);
            let content = (l.w - l.inset[0] - l.inset[2]).max(0.0);
            let available = content;
            let position = match view.position {
                InputBlockPosition::Header => SlotPosition::Header,
                InputBlockPosition::Leading => SlotPosition::Leading,
            };
            let block = view.block;
            let mut style = taffy.style(nodes[child].taffy).clone();
            style.set_width(if position == SlotPosition::Header { length(available) } else { Dimension::max_content() });
            if &style != taffy.style(nodes[child].taffy) {
                taffy.set_style_and_mark_ancestors(nodes[child].taffy, style, std::iter::empty());
            }
            compute_layout(taffy, nodes[child].taffy, Size {
                width: AvailableSpace::Definite(available), height: AvailableSpace::MaxContent,
            }, nodes, text, scale);
            let widget = taffy.box_layout(nodes[child].taffy);
            tables.entry(parent).or_default().push(BlockSlot {
                block, paragraph, position, width: widget.w, height: widget.h, rtl,
            });
        }
        let mut changed = false;
        for (parent, table) in tables {
            let spec = nodes[parent].text.as_mut().unwrap();
            if spec.cold.block_slots == table { continue; }
            spec.cold.block_slots = table;
            taffy.mark_dirty_and_ancestors(nodes[parent].taffy,
                std::iter::successors(nodes[parent].parent, |&p| nodes[p].parent).map(|p| nodes[p].taffy));
            changed = true;
        }
        if !changed { return; }
        compute_layout(taffy, root, room, nodes, text, scale);
    }
}

/// After shaping, move each complete subtree into its reserved slot. Ordinary descendants
/// retain their local layouts, so paint/hit/AX inherit exactly the editor's coordinate chain.
pub(super) fn place(nodes: &mut [Node], scale: f32) {
    for (parent, child) in roots(nodes) {
        let p = &nodes[parent];
        let Some(spec) = &p.text else { continue };
        let Some(layout) = &p.text_layout else { continue };
        let input = p.input.as_ref().unwrap();
        let Some(view) = input.block_views.iter().find(|v| nodes[child].key.as_ref() == Some(&v.key)) else { continue };
        let Some(slot) = spec.cold.block_slots.iter().find(|s| s.block == view.block) else { continue };
        let start = view.start;
        let Some(line) = layout.lines().find(|l| l.text_range().start == start) else { continue };
        let m = line.metrics();
        let content = (p.local.w - p.inset[0] - p.inset[2]).max(0.0);
        let tx = text_x(spec, p.inset[0], p.local.w - p.inset[2], layout.full_width() / scale, crate::el::Flow::of_layout(layout));
        let (x, y) = match slot.position {
            SlotPosition::Header => (tx, p.inset[1] + m.block_min_coord / scale - slot.height),
            SlotPosition::Leading => {
                let x = if slot.rtl { tx + content - slot.width } else { tx + m.inline_min_coord / scale - slot.width };
                (x, p.inset[1] + m.block_min_coord / scale)
            }
        };
        let (px, py) = (p.target[0], p.target[1]);
        let (dx, dy) = (px + x - nodes[child].target[0], py + y - nodes[child].target[1]);
        nodes[child].local.x = x;
        nodes[child].local.y = y;
        for i in child..nodes.len() {
            if i > child && nodes[i].parent.is_none_or(|p| p < child) { break; }
            nodes[i].target[0] += dx;
            nodes[i].target[2] += dx;
            nodes[i].target[1] += dy;
            nodes[i].target[3] += dy;
        }
    }
}

impl<S: 'static> Runtime<S> {
    /// A secure transition is not an ordinary widget exit. Do not retain even a fading
    /// frame of app-owned labels from its formerly rich document.
    pub(super) fn discard_concealed_block_views(&mut self, nodes: &[Node]) {
        let secure: Vec<_> = nodes.iter().filter(|n| n.input.as_ref().is_some_and(|i| i.edit.is_secure()))
            .map(|n| n.path.as_str()).collect();
        if secure.is_empty() { return; }
        for old in &self.nodes {
            if secure.iter().any(|path| old.path.strip_prefix(*path).is_some_and(|r| r.starts_with("/$block-"))) {
                self.scene.unmount(old.id);
                self.grows.remove(&old.id);
            }
        }
    }

    pub(super) fn block_view_escape(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if ev.key != crate::Key::Escape || ev.mods != crate::Mods::default() { return false; }
        let Some(parent) = self.focused_index().and_then(|i| owner(&self.nodes, i)) else { return false };
        let path = self.nodes[parent].path.clone();
        self.set_focus(Some(path.clone()), true);
        tx.scroll_to.push(ScrollReveal::into_view(None, path));
        true
    }

    pub(super) fn block_view_focus_fallback(&self, nodes: &[Node]) -> Option<String> {
        let focused = self.focused_index()?;
        if nodes.iter().any(|n| n.path == self.nodes[focused].path) { return None; }
        let parent = owner(&self.nodes, focused)?;
        nodes.iter().find(|n| n.path == self.nodes[parent].path && n.input.is_some()).map(|n| n.path.clone())
    }
}
