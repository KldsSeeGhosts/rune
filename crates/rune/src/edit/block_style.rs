//! Plain, app-settable defaults for the additive block kinds.

use super::*;
use crate::text::Span;

/// Rendering of headings, task checkboxes and rules. Colors inherit the rich marker color
/// when unset. Sizes and strokes are points; heading sizes multiply the field's text size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockStyle {
    pub heading_scale: [f32; 6],
    pub heading_weight: [f32; 6],
    pub checkbox_size: f32,
    pub checkbox_stroke: f32,
    pub checkbox_color: Option<Color>,
    pub rule_thickness: f32,
    pub rule_color: Option<Color>,
}

impl Default for BlockStyle {
    fn default() -> Self {
        Self {
            heading_scale: [2.0, 1.5, 1.25, 1.125, 1.0, 1.0],
            heading_weight: [600.0; 6],
            checkbox_size: 12.0,
            checkbox_stroke: 1.0,
            checkbox_color: None,
            rule_thickness: 1.0,
            rule_color: None,
        }
    }
}

impl TextEdit {
    pub(crate) fn display_block_kinds(&self, display: &str) -> Vec<BlockKind> {
        if self.block_data.is_empty() || self.secure { return Vec::new(); }
        let mut at = 0;
        display.split('\n').map(|line| {
            let kind = self.block_kind(self.line_of_byte(self.model_index(at)));
            at += line.len() + 1;
            kind
        }).collect()
    }

    pub(crate) fn heading_spans(&self, base: crate::TextStyle, style: BlockStyle) -> Vec<Span> {
        if self.secure { return Vec::new(); }
        let (display, _) = self.display();
        let kinds = self.display_block_kinds(&display);
        let mut at = 0;
        display.split('\n').enumerate().filter_map(|(i, line)| {
            let start = at;
            at += line.len() + 1;
            let BlockKind::Heading { level } = kinds.get(i)? else { return None };
            let level = level.clamp(&1, &6) - 1;
            Some(Span::new(start, start + line.len())
                .size(base.size * style.heading_scale[level as usize].max(0.1))
                .weight(style.heading_weight[level as usize].clamp(1.0, 1000.0)))
        }).collect()
    }
}
