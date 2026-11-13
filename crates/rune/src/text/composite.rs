//! Reserved block-widget room in a single Parley document. No synthetic text or inline boxes.
use super::*;
use std::hash::Hasher;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SlotPosition { Header, Leading }

/// Measured widget dimensions in points. Paragraphs refer to the displayed document.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BlockSlot {
    pub block: crate::BlockId,
    pub paragraph: usize,
    pub position: SlotPosition,
    pub width: f32,
    pub height: f32,
}

pub(crate) fn slots_key(slots: &[BlockSlot]) -> u64 {
    if slots.is_empty() { return 0; }
    let mut h = hash::KeyHasher::default();
    0x626c_6f63_6b73_6c6fu64.hash(&mut h);
    for s in slots {
        (s.block, s.paragraph, s.position, s.width.to_bits(), s.height.to_bits()).hash(&mut h);
    }
    h.finish()
}

impl TextSystem {
    pub(crate) fn layout_composite(
        &mut self, spec: &crate::el::TextSpec, slots: &[BlockSlot], scale: f32,
        width: Option<f32>, align: Alignment,
    ) -> Rc<Layout<Ink>> {
        let base = self.layout_keyed(spec.shape_key(scale), &spec.content, &spec.spans,
            &spec.cold.blocks, &spec.cold.pills, spec.style, scale, width, align,
            spec.wrap_anywhere, !spec.redacted, spec.base());
        if slots.is_empty() { return base; }
        let shape = spec.shape_key(scale).unwrap_or_else(|| spec.fresh_shape_key(scale));
        let key = hash::break_key(shape ^ slots_key(slots), width, align, Some(&spec.cold.blocks));
        if !spec.redacted && let Some(e) = self.layouts.get_mut(&key) {
            e.used = self.generation;
            return e.value.clone();
        }
        let mut layout = (*base).clone();
        line_break::break_lines_with_slots(&mut layout, &spec.cold.blocks, spec.style.size, scale, width, slots);
        layout.align(align, AlignmentOptions::default());
        let layout = Rc::new(layout);
        if !spec.redacted {
            self.layouts.insert(key, Entry::new(layout.clone(), self.generation, spec.content.len(), 0));
        }
        layout
    }
}

impl crate::el::TextSpec {
    /// Parley's height sums line heights, not gaps added by `set_line_y`. Include exactly
    /// the reserved gaps here; ordinary fields keep the upstream measurement byte for byte.
    pub(crate) fn measured_height(&self, layout: &Layout<Ink>, scale: f32) -> f32 {
        let base = spec_height(layout, self.style, scale, self.exact_lines);
        if self.cold.block_slots.is_empty() { return base; }
        let mut extra = 0.0;
        let mut paragraph = 0;
        let mut first = true;
        for line in layout.lines() {
            if first {
                for slot in self.cold.block_slots.iter().filter(|s| s.paragraph == paragraph) {
                    extra += match slot.position {
                        SlotPosition::Header => slot.height * scale,
                        SlotPosition::Leading => (slot.height * scale - line.metrics().line_height).max(0.0),
                    };
                }
            }
            first = line.break_reason() == parley::BreakReason::Explicit;
            if first { paragraph += 1; }
        }
        base + extra
    }
}
