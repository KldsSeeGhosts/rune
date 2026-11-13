//! Ordinary element views attached to logical input blocks.
use super::*;
use crate::BlockId;

/// Where the editor reserves room for a block's non-editable view.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputBlockPosition {
    /// Above the first line, across the block's content width.
    Header,
    /// In the leading inset beside the first line.
    Leading,
}

/// One ordinary element attached by logical identity. Attach with [`El::input_block_view`].
/// The element's own layout, event handlers and accessibility remain ordinary Rune APIs.
pub struct InputBlockView {
    block: BlockId,
    position: InputBlockPosition,
    view: El,
}

impl InputBlockView {
    pub fn new(block: BlockId, position: InputBlockPosition, view: El) -> Self { Self { block, position, view } }
    pub fn header(block: BlockId, view: El) -> Self { Self::new(block, InputBlockPosition::Header, view) }
    pub fn leading(block: BlockId, view: El) -> Self { Self::new(block, InputBlockPosition::Leading, view) }
}

#[derive(Clone, Debug)]
pub(crate) struct BlockViewSpec {
    pub block: BlockId,
    pub position: InputBlockPosition,
    pub key: String,
    pub start: usize,
}

impl InputSpec {
    pub(crate) fn owns_block_view(&self, key: Option<&str>) -> bool {
        key.is_some_and(|key| self.block_views.iter().any(|v| v.key == key))
    }
}

impl El {
    /// Attach or replace one view for `block`. Missing blocks, secure and single-line fields
    /// ignore the view. Registration order does not affect document placement or Tab order.
    ///
    /// Headers take the full block width, including its text inset, and their measured height. Leading
    /// views take their natural width; the editor expands the leading inset when necessary.
    /// A leading view replaces the default list or task marker on that line.
    /// Neither view inserts text. Pointer presses preserve the editor's caret and composition.
    /// Tab visits the editor, then views in document order; `focusable(false)` opts a view out.
    /// Escape from a focused view returns to the editor without changing its selection.
    pub fn input_block_view(mut self, mut view: InputBlockView) -> Self {
        let Some(input) = self.cold.get_mut_if_some().and_then(|c| c.input.as_deref_mut()) else { return self };
        if input.single_line || input.edit.is_secure() { return self; }
        let Some(text) = &self.text else { return self };
        let Some(start) = input.edit.block_view_start(view.block, &text.content) else { return self };
        let key = format!("$block-{}", view.block.get());
        input.block_views.retain(|v| v.block != view.block);
        input.block_views.push(BlockViewSpec { block: view.block, position: view.position, key: key.clone(), start });
        input.block_views.sort_by_key(|v| v.start);
        let order: Vec<_> = input.block_views.iter().map(|v| v.key.clone()).collect();
        if !view.view.cold.focus.opt_out { view.view = view.view.focusable(true); }
        let mut wrapper = col().key(&key).items_start().instant_changes().child(view.view);
        // The entire reserved slot consumes presses, including its empty padding.
        wrapper.handlers.on_press = Some(Rc::new(|_, _| {}));
        self.children.retain(|c| c.key.as_deref() != Some(key.as_str()));
        self.children.push(wrapper);
        self.children.sort_by_key(|c| order.iter().position(|k| c.key.as_ref() == Some(k)));
        if self.vis.clip.is_none() { self = self.clip_soft(0.0); }
        self
    }

    pub(crate) fn clear_input_block_views(&mut self) {
        let Some(input) = self.cold.get_mut_if_some().and_then(|c| c.input.as_deref_mut()) else { return };
        let keys: Vec<_> = input.block_views.drain(..).map(|v| v.key).collect();
        self.children.retain(|c| c.key.as_ref().is_none_or(|k| !keys.contains(k)));
        if let Some(t) = &mut self.text { t.cold.block_slots.clear(); }
    }
}
