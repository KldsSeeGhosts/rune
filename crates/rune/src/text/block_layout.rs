//! Optional heading metrics and rule whitespace, separate from authored document data.
use super::*;
use std::hash::Hasher;
use std::ops::Range;

#[derive(Clone, Copy)]
pub(super) struct ParagraphGap { pub paragraph: usize, pub before: f32, pub after: f32 }

impl crate::el::TextSpec {
    pub(crate) fn heading_heights(&self) -> Vec<(Range<usize>, f32, f32)> {
        if self.cold.block_style.heading_line_height.iter().all(Option::is_none) { return Vec::new(); }
        let mut at = 0;
        self.content.split('\n').enumerate().filter_map(|(i, line)| {
            let start = at;
            at += line.len() + 1;
            let crate::BlockKind::Heading { level } = self.cold.block_kinds.get(i)? else { return None };
            let level = (level.clamp(&1, &6) - 1) as usize;
            let height = self.cold.block_style.heading_line_height[level]
                .filter(|v| v.is_finite() && *v > 0.0)?;
            let size = self.style.size * self.cold.block_style.heading_scale[level].max(0.1);
            let height = height * size;
            (height.is_finite() && height > 0.0).then_some((start..at.min(self.content.len()), height, size))
        }).collect()
    }

    pub(crate) fn heading_height_key(&self, key: u64) -> u64 {
        let heights = self.heading_heights();
        if heights.is_empty() { return key; }
        let mut h = hash::KeyHasher::default();
        key.hash(&mut h);
        for (r, height, size) in heights { (r, height.to_bits(), size.to_bits()).hash(&mut h); }
        self.heading_tail().map(|(size, height)| (size.to_bits(), height.to_bits())).hash(&mut h);
        h.finish()
    }

    /// Empty paragraphs have no glyph whose style can carry their own line metrics.
    pub(crate) fn heading_tail(&self) -> Option<(f32, f32)> {
        if !self.content.is_empty() && !self.content.ends_with('\n') { return None; }
        let paragraph = self.content.bytes().filter(|&b| b == b'\n').count();
        if let Some(crate::BlockKind::Heading { level }) = self.cold.block_kinds.get(paragraph) {
            let level = (level.clamp(&1, &6) - 1) as usize;
            if let Some(multiplier) = self.cold.block_style.heading_line_height[level].filter(|v| v.is_finite() && *v > 0.0) {
                let size = self.style.size * self.cold.block_style.heading_scale[level].max(0.1);
                if (size * multiplier).is_finite() && size > 0.0 { return Some((size, size * multiplier)); }
            }
        }
        Some((self.style.size, self.style.line_px()))
    }

    pub(crate) fn block_gap_key(&self) -> u64 { gaps_key(&self.rule_gaps()) }

    pub(super) fn rule_gaps(&self) -> Vec<ParagraphGap> {
        let [before, after] = self.cold.block_style.rule_margins();
        if before == 0.0 && after == 0.0 { return Vec::new(); }
        self.cold.block_kinds.iter().enumerate().filter_map(|(paragraph, kind)|
            (*kind == crate::BlockKind::Rule).then_some(ParagraphGap { paragraph, before, after })).collect()
    }
}

pub(super) fn gaps_key(gaps: &[ParagraphGap]) -> u64 {
    if gaps.is_empty() { return 0; }
    let mut h = hash::KeyHasher::default();
    0x7275_6c65_6761_7073u64.hash(&mut h);
    for g in gaps { (g.paragraph, g.before.to_bits(), g.after.to_bits()).hash(&mut h); }
    h.finish()
}
