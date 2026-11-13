//! Default task/rule hit targets. These use the text layout, not overlay elements.

use super::*;

pub(super) fn block_click(
    edit: &mut crate::TextEdit,
    layout: &Layout<Ink>,
    byte: usize,
    x: f32,
    y: f32,
    scale: f32,
    font_size: f32,
    style: crate::BlockStyle,
    custom_marker: bool,
) -> bool {
    let [before, after] = style.rule_margins();
    if before > 0.0 || after > 0.0 {
        for visual in layout.lines() {
            let at = edit.model_index(visual.text_range().start);
            if edit.block_kind(edit.line_of_byte(at)) != crate::BlockKind::Rule { continue; }
            let m = visual.metrics();
            if y >= m.block_min_coord / scale - before && y <= m.block_max_coord / scale + after {
                return edit.select_rule_at(at);
            }
        }
    }
    let line = edit.line_of_byte(byte);
    match edit.block_kind(line) {
        crate::BlockKind::Rule => edit.select_rule_at(byte),
        crate::BlockKind::Task { .. } if custom_marker => false,
        crate::BlockKind::Task { checked, .. } => {
            let start = edit.display_index(edit.line_range(line).start);
            let Some(first) = layout.lines().find(|l| l.text_range().start == start) else { return false };
            let d = style.checkbox_size.max(0.0);
            let left = edit.blocks()[line].indent_pt(font_size) - d - 5.0;
            let top = first.metrics().baseline / scale - font_size * 0.34 - d * 0.5;
            if x >= left && x <= left + d && y >= top && y <= top + d {
                edit.set_task_checked(line, !checked);
                true
            } else { false }
        }
        _ => false,
    }
}
