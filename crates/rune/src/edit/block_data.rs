//! Additive block kinds and authored source, alongside the legacy `Copy` block enum.

use super::*;

/// A block's complete kind. [`Block`] stays exhaustive and `Copy` for existing apps.
/// `blocks()` exposes a heading or rule as a paragraph, and a task as a bullet.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BlockKind {
    #[default]
    Paragraph,
    Bullet(u8),
    Numbered(u8),
    Quote,
    Code,
    Heading { level: u8 },
    Task { level: u8, checked: bool },
    /// A rule occupies an empty paragraph. Atomic interaction is separate from its kind.
    Rule,
}

impl From<Block> for BlockKind {
    fn from(block: Block) -> Self {
        match block {
            Block::Paragraph => Self::Paragraph,
            Block::Bullet(l) => Self::Bullet(l),
            Block::Numbered(l) => Self::Numbered(l),
            Block::Quote => Self::Quote,
            Block::Code => Self::Code,
        }
    }
}

impl BlockKind {
    pub(super) fn legacy(self) -> Block {
        match self {
            Self::Paragraph | Self::Heading { .. } | Self::Rule => Block::Paragraph,
            Self::Bullet(l) | Self::Task { level: l, .. } => Block::Bullet(l),
            Self::Numbered(l) => Block::Numbered(l),
            Self::Quote => Block::Quote,
            Self::Code => Block::Code,
        }
    }

    pub(super) fn normalized(self) -> Self {
        match self {
            Self::Heading { level } => Self::Heading { level: level.clamp(1, 6) },
            other => other,
        }
    }
}

/// Exact authored source. `None` means unspecified; `Some("")` is an authored empty string.
/// Rune retains these strings, without interpreting or normalizing Markdown grammar.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BlockAttributes {
    indent: Option<String>,
    marker: Option<String>,
    space: Option<String>,
    marker_space: Option<String>,
    content_space: Option<String>,
    quote_prefix: Option<String>,
    heading_space: Option<String>,
    rule_source: Option<String>,
    code: Option<CodeAttributes>,
}

macro_rules! source_attribute {
    ($get:ident, $with:ident) => {
        impl BlockAttributes {
            pub fn $get(&self) -> Option<&str> { self.$get.as_deref() }
            pub fn $with(mut self, value: impl Into<String>) -> Self {
                self.$get = Some(value.into());
                self
            }
        }
    };
}

source_attribute!(indent, with_indent);
source_attribute!(marker, with_marker);
source_attribute!(space, with_space);
source_attribute!(marker_space, with_marker_space);
source_attribute!(content_space, with_content_space);
source_attribute!(quote_prefix, with_quote_prefix);
source_attribute!(heading_space, with_heading_space);
source_attribute!(rule_source, with_rule_source);

impl BlockAttributes {
    pub fn code(&self) -> Option<&CodeAttributes> { self.code.as_ref() }
    pub fn with_code(mut self, code: CodeAttributes) -> Self { self.code = Some(code); self }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(super) struct BlockData {
    pub(super) kind: Option<BlockKind>,
    pub(super) attributes: BlockAttributes,
    /// Only used while importing/exporting HTML. The live model owns `block_ids`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) id: Option<BlockId>,
}

impl BlockData {
    pub(super) fn bytes(&self) -> usize {
        let a = &self.attributes;
        std::mem::size_of::<Self>() + [
            &a.indent, &a.marker, &a.space, &a.marker_space, &a.content_space,
            &a.quote_prefix, &a.heading_space, &a.rule_source,
        ].iter().filter_map(|s| s.as_ref()).map(|s| s.len()).sum::<usize>()
            + a.code.as_ref().map_or(0, CodeAttributes::bytes)
    }
}

static EMPTY: BlockAttributes = BlockAttributes {
    indent: None, marker: None, space: None, marker_space: None, content_space: None,
    quote_prefix: None, heading_space: None, rule_source: None, code: None,
};

impl TextEdit {
    /// Full kind of a newline-delimited line, including headings, tasks and rules.
    pub fn block_kind(&self, line: usize) -> BlockKind {
        self.block_data.get(line).and_then(|d| d.kind)
            .unwrap_or_else(|| self.blocks.get(line).copied().unwrap_or_default().into())
    }

    /// Authored source on a line, including an empty line. Out-of-range lines are unspecified.
    pub fn block_attributes(&self, line: usize) -> &BlockAttributes {
        self.block_data.get(line).map_or(&EMPTY, |d| &d.attributes)
    }

    /// Set authored source as one undo step. Does not change the line's kind.
    pub fn set_block_attributes(&mut self, line: usize, attributes: BlockAttributes) {
        if self.secure { return; }
        self.sync_invariants();
        if line >= self.blocks.len() { return; }
        self.break_history_group();
        let before = self.snapshot();
        self.block_data.resize(self.blocks.len(), BlockData::default());
        let id = self.block_ids[line];
        for (i, &block_id) in self.block_ids.iter().enumerate() {
            if block_id == id { self.block_data[i].attributes = attributes.clone(); }
        }
        self.record_change(before, EditKind::Other);
    }

    /// Set a line's full kind as one undo step. Existing block allowlists check its legacy
    /// projection. A rule needs an empty line; a nonempty line is left unchanged.
    pub fn set_block_kind(&mut self, line: usize, kind: BlockKind) {
        if self.secure || self.is_single_line() || !self.allows_block(kind.legacy()) { return; }
        self.sync_invariants();
        if line >= self.blocks.len() || (kind == BlockKind::Rule && !self.line_range(line).is_empty()) { return; }
        self.break_history_group();
        let before = self.snapshot();
        let kind = kind.normalized();
        self.block_data.resize(self.blocks.len(), BlockData::default());
        self.block_data[line].kind = Some(kind);
        self.blocks[line] = kind.legacy();
        self.repair_block_ids();
        self.normalize_marks();
        self.record_change(before, EditKind::Other);
    }

    /// Change a task checkbox, recording undo. Other kinds are unchanged.
    pub fn set_task_checked(&mut self, line: usize, checked: bool) {
        if let BlockKind::Task { level, .. } = self.block_kind(line) {
            self.set_block_kind(line, BlockKind::Task { level, checked });
        }
    }

    pub(super) fn sync_block_data(&mut self) {
        if self.secure { self.block_data.clear(); return; }
        if self.block_data.is_empty() { return; }
        self.block_data.resize(self.blocks.len(), BlockData::default());
        let content: Vec<bool> = if self.block_data.iter().any(|d| d.kind == Some(BlockKind::Rule)) {
            self.text.split('\n').map(|s| !s.is_empty()).collect()
        } else { Vec::new() };
        for (i, (d, &block)) in self.block_data.iter_mut().zip(&self.blocks).enumerate() {
            if d.kind == Some(BlockKind::Rule) && content.get(i) == Some(&true) { d.kind = None; }
            if d.kind.is_some_and(|k| k.legacy() != block) {
                // Legacy APIs changed a block. Never let a stale extended kind override it.
                d.kind = match d.kind {
                    Some(BlockKind::Task { checked, .. }) if block.is_list() =>
                        Some(BlockKind::Task { level: block.list_level().unwrap_or(0), checked }),
                    _ => None,
                };
            }
        }
    }

    /// The same left/right survivor rule as the legacy block replacement engine.
    pub(super) fn map_block_data(&mut self, a: usize, b: usize, added: usize, right_survives: bool, next: Option<Block>, soft: bool) {
        if self.block_data.is_empty() { return; }
        let survivor = self.block_data[if right_survives { b } else { a }].clone();
        let mut continuation = survivor.clone();
        if !soft {
            continuation.kind = match (continuation.kind, next) {
                (Some(BlockKind::Heading { .. } | BlockKind::Rule), Some(Block::Paragraph)) => None,
                (Some(BlockKind::Task { level, .. }), Some(Block::Bullet(_))) =>
                    Some(BlockKind::Task { level, checked: false }),
                (kind, Some(block)) if kind.is_some_and(|k| k.legacy() != block) => None,
                (kind, _) => kind,
            };
        }
        self.block_data[a] = survivor;
        self.block_data.splice(a + 1..b + 1, std::iter::repeat_n(continuation, added));
    }

    pub(super) fn clear_extended_kinds(&mut self, first: usize, last: usize) {
        for d in self.block_data.iter_mut().take(last + 1).skip(first) { d.kind = None; }
    }
}
