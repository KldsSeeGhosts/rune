//! Atomic object selection within the existing single text document.

use super::*;
use crate::machine::Effects;

/// A text selection retains its direction. A rule is selected as one object even though its
/// logical content range is empty.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorSelection {
    Text { anchor: usize, focus: usize },
    Block(BlockId),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum SelectionState {
    #[default]
    Text,
    Block(BlockId),
}

enum SelectionEvent { Text, Block(BlockId) }

/// Selecting an object changes the selection mode, not the document.
fn step(state: &mut SelectionState, event: SelectionEvent) -> Effects<()> {
    *state = match event {
        SelectionEvent::Text => SelectionState::Text,
        SelectionEvent::Block(id) => SelectionState::Block(id),
    };
    Effects::none()
}

impl TextEdit {
    pub fn selection(&self) -> EditorSelection {
        if let Some((id, _)) = self.selected_rule() { return EditorSelection::Block(id); }
        EditorSelection::Text {
            anchor: Self::clamp_char_boundary(&self.text, self.anchor),
            focus: Self::clamp_char_boundary(&self.text, self.focus),
        }
    }

    /// Select a rule by identity. False for missing ids or non-rule content.
    pub fn select_block(&mut self, id: BlockId) -> bool {
        if self.secure { return false; }
        self.sync_invariants();
        let Some(line) = self.block_ids.iter().position(|&i| i == id) else { return false };
        if self.block_kind(line) != BlockKind::Rule || !self.line_range(line).is_empty() { return false; }
        self.explicit_selection();
        let at = self.line_range(line).start;
        self.anchor = at;
        self.focus = at;
        self.typing = Typing::Inherited;
        let _ = step(&mut self.block_selection, SelectionEvent::Block(id));
        self.break_history_group();
        true
    }

    pub(crate) fn selected_rule(&self) -> Option<(BlockId, usize)> {
        let SelectionState::Block(id) = self.block_selection else { return None };
        let line = self.block_ids.iter().position(|&i| i == id)?;
        let r = self.line_range(line);
        (self.block_kind(line) == BlockKind::Rule && r.is_empty()
            && self.anchor == r.start && self.focus == r.start).then_some((id, line))
    }

    pub(super) fn clear_block_selection(&mut self) {
        let _ = step(&mut self.block_selection, SelectionEvent::Text);
    }

    pub(crate) fn select_rule_at(&mut self, byte: usize) -> bool {
        if !self.block_data.iter().any(|d| d.kind == Some(BlockKind::Rule)) { return false; }
        let line = self.line_of_byte(byte);
        if self.block_kind(line) != BlockKind::Rule { return false; }
        self.block_ids.get(line).copied().is_some_and(|id| self.select_block(id))
    }

    /// A selection replacement at a rule changes its kind or removes its whole paragraph.
    pub(super) fn replace_rule(&mut self, range: (usize, usize), text: &str, kind: EditKind) -> bool {
        if !self.block_data.iter().any(|d| d.kind == Some(BlockKind::Rule)) { return false; }
        if range != self.range() { return false; }
        if self.selected_rule().is_none() && (range.0 != range.1 || !self.select_rule_at(range.0)) { return false; }
        let Some((_, line)) = self.selected_rule() else { return false };
        self.break_history_group();
        let before = self.snapshot();
        let r = self.line_range(line);
        self.clear_block_selection();
        self.typing = Typing::Inherited;
        if !text.is_empty() || self.blocks.len() == 1 {
            self.blocks[line] = Block::Paragraph;
            self.block_ids[line] = BlockId::fresh();
            self.block_data[line] = BlockData::default();
            self.apply_text_and_attr_replace((r.start, r.end), text, &[], None, false);
        } else if r.end < self.text.len() {
            self.apply_text_and_attr_replace((r.start, r.end + 1), "", &[], None, false);
        } else {
            // If both the preceding paragraph and rule are empty, the replacement engine's
            // right-survivor rule must still retain the preceding paragraph's identity/source.
            self.blocks[line] = self.blocks[line - 1];
            self.block_ids[line] = self.block_ids[line - 1];
            self.block_data[line] = self.block_data.get(line - 1).cloned().unwrap_or_default();
            self.apply_text_and_attr_replace((r.start - 1, r.end), "", &[], None, false);
        }
        self.sync_invariants();
        self.record_change(before, kind);
        true
    }

    pub(super) fn rule_key(&mut self, event: &KeyEvent) -> bool {
        if !self.block_data.iter().any(|d| d.kind == Some(BlockKind::Rule)) { return false; }
        if self.secure || self.is_single_line() || self.is_composing() { return false; }
        if self.selected_rule().is_some() && matches!(event.key, Key::Backspace | Key::Delete) {
            self.replace_rule(self.range(), "", EditKind::Other);
            return true;
        }
        if event.mods.shift || event.mods.cmd || event.mods.ctrl || event.mods.alt { return false; }
        if let Some((_, line)) = self.selected_rule() {
            match event.key {
                Key::Backspace | Key::Delete => { self.replace_rule(self.range(), "", EditKind::Other); true }
                Key::Left | Key::Up => {
                    if line > 0 { self.set_caret(self.line_range(line).start - 1); }
                    true
                }
                Key::Right | Key::Down | Key::Enter => {
                    let end = self.line_range(line).end;
                    if end < self.text.len() {
                        self.set_caret(end + 1);
                    } else {
                        let before = self.snapshot();
                        self.clear_block_selection();
                        self.apply_text_and_attr_replace((end, end), "\n", &[], Some(Block::Paragraph), false);
                        self.record_change(before, EditKind::Other);
                    }
                    true
                }
                _ => false,
            }
        } else {
            let (a, b) = self.range();
            if a != b { return false; }
            let line = self.line_of_byte(a);
            if self.block_kind(line) == BlockKind::Rule
                && matches!(event.key, Key::Left | Key::Right | Key::Up | Key::Down | Key::Backspace | Key::Delete)
            { return self.select_block(self.block_ids[line]); }
            let target = match event.key {
                Key::Backspace if a == self.line_range(line).start && line > 0 => Some(line - 1),
                Key::Delete if a == self.line_range(line).end && line + 1 < self.blocks.len() => Some(line + 1),
                _ => None,
            };
            target.is_some_and(|line| self.block_kind(line) == BlockKind::Rule && self.select_block(self.block_ids[line]))
        }
    }

    pub(super) fn finish_rule_navigation(&mut self, event: &KeyEvent) {
        if self.anchor == self.focus && !event.mods.shift && matches!(event.key, Key::Left | Key::Right | Key::Up | Key::Down | Key::Home | Key::End) {
            self.select_rule_at(self.focus);
        }
    }
}
