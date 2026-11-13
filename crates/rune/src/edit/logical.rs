//! Logical block identity over the existing newline-delimited model.

use portable_atomic::{AtomicU64, Ordering};
use super::*;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Opaque identity, independent of content, line number and position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct BlockId(u64);

impl BlockId {
    pub fn get(self) -> u64 { self.0 }

    pub(super) fn fresh() -> Self { Self(NEXT_ID.fetch_add(1, Ordering::Relaxed)) }

    pub(super) fn imported(self) -> Option<Self> {
        if self.0 == 0 || self.0 >= u64::MAX / 2 { return None; }
        NEXT_ID.fetch_max(self.0 + 1, Ordering::Relaxed);
        Some(self)
    }
}

/// A complete logical block. The range excludes its separator from the following block,
/// but includes newlines within it. An empty block has an empty range and still has an id.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct LogicalBlock {
    pub id: BlockId,
    pub range: Range<usize>,
    pub kind: BlockKind,
    pub attributes: BlockAttributes,
}

/// A logical setter requires whole physical lines and an empty, single line for a rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockRangeError;

impl std::fmt::Display for BlockRangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a logical block must cover whole lines; a rule must cover one empty line")
    }
}

impl std::error::Error for BlockRangeError {}

/// Exact fence source, including the complete info string and an optional closing fence.
/// These strings are not parsed as Markdown. An absent close differs from an empty one.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CodeAttributes {
    fence: String,
    info: String,
    close: Option<String>,
}

impl Default for CodeAttributes {
    fn default() -> Self { Self::new("```", "") }
}

impl CodeAttributes {
    pub fn new(fence: impl Into<String>, info: impl Into<String>) -> Self {
        let fence = fence.into();
        let close = Some(format!("\n{fence}"));
        Self { fence, info: info.into(), close }
    }
    pub fn with_close(mut self, close: Option<String>) -> Self { self.close = close; self }
    pub fn fence(&self) -> &str { &self.fence }
    pub fn info(&self) -> &str { &self.info }
    pub fn close(&self) -> Option<&str> { self.close.as_deref() }
    pub(super) fn bytes(&self) -> usize {
        self.fence.len() + self.info.len() + self.close.as_ref().map_or(0, String::len)
    }
}

impl TextEdit {
    /// Enumerate logical blocks. Use model editing methods to retain identity. Assigning the
    /// public `text` field directly cannot map attributes or identities through the change.
    /// Authored/logical setters opt HTML into identity persistence; legacy HTML stays unchanged.
    /// Secure fields expose no logical blocks.
    pub fn logical_blocks(&self) -> impl Iterator<Item = LogicalBlock> {
        let mut out: Vec<LogicalBlock> = Vec::new();
        if self.secure { return out.into_iter(); }
        let e = self.synced();
        for p in e.paragraphs() {
            let id = e.block_ids[p.index];
            if let Some(last) = out.last_mut().filter(|b| b.id == id) {
                last.range.end = p.range.end;
            } else {
                out.push(LogicalBlock {
                    id, range: p.range, kind: e.block_kind(p.index),
                    attributes: e.block_attributes(p.index).clone(),
                });
            }
        }
        out.into_iter()
    }

    /// Make a new logical block with its own identity, recording one undo step.
    /// Use `update_logical_block` to change an existing block without replacing its identity.
    pub fn set_logical_block(&mut self, range: Range<usize>, kind: BlockKind, attributes: BlockAttributes) -> Result<BlockId, BlockRangeError> {
        if self.secure || self.is_single_line() || !self.allows_block(kind.legacy()) { return Err(BlockRangeError); }
        self.sync_invariants();
        let (first, last) = self.logical_lines(&range, kind)?;
        let id = BlockId::fresh();
        self.write_logical_block(first, last, id, kind, attributes);
        Ok(id)
    }

    /// Set kind and source on an existing logical block as one undo step. False when the id
    /// no longer exists, the kind is disallowed, or a rule would replace nonempty content.
    pub fn update_logical_block(&mut self, id: BlockId, kind: BlockKind, attributes: BlockAttributes) -> bool {
        if self.secure || self.is_single_line() || !self.allows_block(kind.legacy()) { return false; }
        self.sync_invariants();
        let Some(first) = self.block_ids.iter().position(|&i| i == id) else { return false };
        let last = self.block_ids.iter().rposition(|&i| i == id).unwrap();
        if self.logical_lines(&(self.line_range(first).start..self.line_range(last).end), kind).is_err() { return false; }
        self.write_logical_block(first, last, id, kind, attributes);
        true
    }

    fn logical_lines(&self, range: &Range<usize>, kind: BlockKind) -> Result<(usize, usize), BlockRangeError> {
        if range.start > range.end || range.end > self.text.len()
            || !self.text.is_char_boundary(range.start) || !self.text.is_char_boundary(range.end) { return Err(BlockRangeError); }
        let first = self.line_of_byte(range.start);
        let last = self.line_of_byte(range.end);
        if self.line_range(first).start != range.start || self.line_range(last).end != range.end
            || (kind == BlockKind::Rule && (first != last || !range.is_empty())) { return Err(BlockRangeError); }
        Ok((first, last))
    }

    fn write_logical_block(&mut self, first: usize, last: usize, id: BlockId, kind: BlockKind, attributes: BlockAttributes) {
        self.break_history_group();
        let before = self.snapshot();
        self.block_data.resize(self.blocks.len(), Default::default());
        let kind = kind.normalized();
        for i in first..=last {
            self.block_ids[i] = id;
            self.blocks[i] = kind.legacy();
            self.block_data[i] = BlockData { kind: Some(kind), attributes: attributes.clone(), ..Default::default() };
        }
        self.soft_breaks.retain(|&i| i as usize <= first || i as usize > last);
        if kind != BlockKind::Code { self.soft_breaks.extend((first + 1..=last).map(|i| i as u32)); }
        self.repair_block_ids();
        self.sync_invariants();
        self.record_change(before, EditKind::Other);
    }

    pub(super) fn sync_block_ids(&mut self) {
        if self.secure { self.block_ids.clear(); return; }
        self.block_ids.resize_with(self.blocks.len(), BlockId::fresh);
    }

    /// No id may recur in disjoint blocks, or cover different kinds/source attributes.
    pub(super) fn repair_block_ids(&mut self) {
        self.sync_block_ids();
        let mut seen = std::collections::HashSet::new();
        let mut previous: Option<(BlockId, BlockId, BlockKind, BlockAttributes)> = None;
        for i in 0..self.block_ids.len() {
            let original = self.block_ids[i];
            let kind = self.block_kind(i);
            let attributes = self.block_attributes(i).clone();
            let id = if let Some((old, mapped, k, a)) = &previous
                && *old == original && *k == kind && *a == attributes
                && (kind == BlockKind::Code || self.soft_breaks.contains(&(i as u32))) { *mapped }
            else if seen.insert(original) { original }
            else { BlockId::fresh() };
            self.block_ids[i] = id;
            previous = Some((original, id, kind, attributes));
        }
    }

    pub(super) fn group_legacy_blocks(&mut self) {
        self.sync_block_ids();
        for i in 1..self.block_ids.len() {
            if self.blocks[i] == self.blocks[i - 1]
                && (self.blocks[i] == Block::Code || self.soft_breaks.contains(&(i as u32)))
                && self.block_attributes(i) == self.block_attributes(i - 1)
            { self.block_ids[i] = self.block_ids[i - 1]; }
            else if self.block_ids[i] == self.block_ids[i - 1] { self.block_ids[i] = BlockId::fresh(); }
        }
        self.repair_block_ids();
    }

    pub(super) fn map_block_ids(&mut self, a: usize, b: usize, text: &str, next: Option<Block>, soft: bool) {
        let removed = count_newlines(&self.text.as_bytes()[a..b]);
        let added = count_newlines(text.as_bytes());
        if removed == 0 && added == 0 { return; }
        let first = self.line_of_byte(a);
        let last = first + removed;
        let right_survives = first < last && self.line_range(first).is_empty()
            && a == self.line_range(first).start && b == self.line_range(last).start;
        let survivor = if right_survives { last } else { first };
        let id = self.block_ids[survivor];
        let continues = soft || next == Some(Block::Code)
            || (next.is_none() && self.blocks[survivor] == Block::Code);
        self.block_ids[first] = id;
        self.block_ids.splice(first + 1..last + 1,
            (0..added).map(|_| if continues { id } else { BlockId::fresh() }));
    }

    pub(super) fn paste_block_ids(&mut self, frag: &TextEdit, line: usize, take_first: bool) {
        let mut remap = std::collections::HashMap::new();
        for (k, &id) in frag.block_ids.iter().enumerate() {
            if k == 0 && !take_first { continue; }
            if let Some(slot) = self.block_ids.get_mut(line + k) {
                *slot = *remap.entry(id).or_insert_with(BlockId::fresh);
            }
        }
        self.repair_block_ids();
    }
}
