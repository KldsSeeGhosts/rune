//! HTML's additive authored-line encoding. Ordinary HTML import remains the fallback.

use super::*;
use crate::edit::block_data::BlockData;
use crate::edit::BlockKind;

pub(super) fn serialize(edit: &TextEdit, range: Range<usize>) -> String {
    let mut out = String::new();
    for p in edit.paragraphs() {
        if p.range.end < range.start || p.range.start > range.end { continue; }
        let start = p.range.start.max(range.start);
        let end = p.range.end.min(range.end);
        if start > end || (start == end && !p.range.is_empty() && !range.is_empty()) { continue; }
        let kind = edit.block_kind(p.index);
        let mut data = edit.block_data.get(p.index).cloned().unwrap_or_default();
        data.kind = Some(kind);
        let tag = match kind {
            BlockKind::Heading { level: 1 } => "h1",
            BlockKind::Heading { level: 2 } => "h2",
            BlockKind::Heading { level: 3 } => "h3",
            BlockKind::Heading { level: 4 } => "h4",
            BlockKind::Heading { level: 5 } => "h5",
            BlockKind::Heading { .. } => "h6",
            BlockKind::Code => "pre",
            BlockKind::Quote => "blockquote",
            BlockKind::Rule => "hr",
            _ => "p",
        };
        out.push('<');
        out.push_str(tag);
        out.push_str(" data-rune-line=\"");
        // Struct fields and enum variants are the versioned, inert data format.
        escape_html(&serde_json::to_string(&data).unwrap_or_default(), &mut out);
        out.push_str("\" style=\"white-space:pre-wrap\"");
        if p.soft_break { out.push_str(" data-rune-soft=\"true\""); }
        out.push('>');
        if kind == BlockKind::Rule { continue; }
        if kind == BlockKind::Code {
            out.push_str("<code>");
            escape_html(&edit.text[start..end], &mut out);
            out.push_str("</code>");
        } else {
            serialize_inline(edit, start..end, &mut out);
        }
        out.push_str("</");
        out.push_str(tag);
        out.push('>');
    }
    out
}

pub(super) fn read(attrs: &str) -> Option<BlockData> {
    let encoded = extract_attr(attrs, "data-rune-line")?;
    let mut data: BlockData = serde_json::from_str(&encoded).ok()?;
    data.kind = data.kind.map(BlockKind::normalized);
    Some(data)
}
