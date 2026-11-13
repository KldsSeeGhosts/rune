use super::*;

fn code() -> BlockAttributes {
    BlockAttributes::default().with_code(
        CodeAttributes::new("~~~~", "rust title=\"<&>\"  ").with_close(Some("\n~~~~~ \t".into())),
    )
}

#[test]
fn equal_adjacent_code_blocks_have_distinct_identity_even_when_empty() {
    let mut e = TextEdit::new("same\nsame\n").rich();
    let left = e.set_logical_block(0..4, BlockKind::Code, code()).unwrap();
    let right = e.set_logical_block(5..9, BlockKind::Code, code()).unwrap();
    let empty = e.set_logical_block(10..10, BlockKind::Code, code()).unwrap();
    assert_ne!(left, right);
    assert_ne!(right, empty);
    let blocks = e.logical_blocks().collect::<Vec<_>>();
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].range, 0..4);
    assert_eq!(blocks[1].range, 5..9);
    assert_eq!(blocks[2].range, 10..10);
    assert_eq!(blocks[2].attributes, code());
    assert_eq!(blocks[2].id, empty);
}

#[test]
fn code_identity_survives_insert_split_join_undo_and_redo() {
    let mut e = TextEdit::new("one\ntwo\nend").rich();
    let id = e.set_logical_block(0..7, BlockKind::Code, code()).unwrap();
    let end = e.logical_blocks().last().unwrap().id;
    e.clear_history();
    e.set_caret(2);
    e.insert("\n");
    let blocks = e.logical_blocks().collect::<Vec<_>>();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].id, id);
    assert_eq!(blocks[0].range, 0..8);
    assert_eq!(blocks[1].id, end);
    assert_eq!(e.block_attributes(2), &code());
    e.set_selection(2, 3);
    e.insert("");
    assert_eq!(e.logical_blocks().next().unwrap().id, id);
    e.undo();
    assert_eq!(e.logical_blocks().next().unwrap().range, 0..8);
    e.undo();
    assert_eq!(e.logical_blocks().next().unwrap().range, 0..7);
    e.redo();
    assert_eq!(e.logical_blocks().next().unwrap().id, id);
    e.redo();
    assert_eq!(e.logical_blocks().last().unwrap().id, end);
}

#[test]
fn paragraph_splits_get_new_ids_and_empty_prefix_join_keeps_the_right_id() {
    let mut e = TextEdit::new("\nbody").rich();
    let id = e.logical_blocks().last().unwrap().id;
    e.set_selection(0, 1);
    e.insert("");
    assert_eq!(e.logical_blocks().next().unwrap().id, id);
    e.set_caret(2);
    e.insert("\n");
    let blocks = e.logical_blocks().collect::<Vec<_>>();
    assert_eq!(blocks[0].id, id);
    assert_ne!(blocks[0].id, blocks[1].id);
    e.undo();
    assert_eq!(e.logical_blocks().next().unwrap().id, id);
}

#[test]
fn html_preserves_fences_ids_empty_blocks_and_an_absent_close() {
    let mut e = TextEdit::new("same\nsame\n").rich();
    e.set_logical_block(0..4, BlockKind::Code, code()).unwrap();
    e.set_logical_block(5..9, BlockKind::Code, code()).unwrap();
    e.set_logical_block(
        10..10, BlockKind::Code,
        BlockAttributes::default().with_code(CodeAttributes::new("`````", "\tunknown key=val").with_close(None)),
    ).unwrap();
    let html = e.to_html();
    let rt = TextEdit::from_html(&html);
    assert_eq!(rt.text, e.text);
    assert_eq!(rt.logical_blocks().collect::<Vec<_>>(), e.logical_blocks().collect::<Vec<_>>());
    assert_eq!(rt.block_attributes(2).code().unwrap().close(), None);
    let clean = crate::clipboard::sanitize_html(&html);
    assert_eq!(TextEdit::from_html(&clean).logical_blocks().collect::<Vec<_>>(), rt.logical_blocks().collect::<Vec<_>>());
}

#[test]
fn copied_code_is_reidentified_without_merging_equal_fences() {
    let mut e = TextEdit::new("a\nb").rich();
    let a = e.set_logical_block(0..1, BlockKind::Code, code()).unwrap();
    let b = e.set_logical_block(2..3, BlockKind::Code, code()).unwrap();
    e.set_selection(0, 3);
    let html = e.selected_html();
    let mut pasted = TextEdit::new("prefix\n").rich();
    pasted.paste_html(&html, "a\nb");
    assert_eq!(pasted.text, "prefix\na\nb");
    let blocks = pasted.logical_blocks().collect::<Vec<_>>();
    assert_ne!(blocks[1].id, a);
    assert_ne!(blocks[2].id, b);
    assert_ne!(blocks[1].id, blocks[2].id);
    assert_eq!(blocks[1].attributes, code());
    pasted.undo();
    assert_eq!(pasted.text, "prefix\n");
    pasted.redo();
    assert_eq!(pasted.logical_blocks().collect::<Vec<_>>(), blocks);
}

#[test]
fn updating_a_code_blocks_attributes_keeps_its_id_and_other_blocks() {
    let mut e = TextEdit::new("same\nsame").rich();
    let id = e.set_logical_block(0..4, BlockKind::Code, code()).unwrap();
    let other = e.set_logical_block(5..9, BlockKind::Code, code()).unwrap();
    let attributes = BlockAttributes::default().with_code(CodeAttributes::new("```", "python suffix"));
    assert!(e.update_logical_block(id, BlockKind::Code, attributes.clone()));
    assert_eq!(e.logical_blocks().next().unwrap().id, id);
    assert_eq!(e.block_attributes(0), &attributes);
    assert_eq!(e.logical_blocks().last().unwrap().id, other);
    assert_eq!(e.block_attributes(1), &code());
    e.undo();
    assert_eq!(e.block_attributes(0), &code());
    e.redo();
    assert_eq!(e.block_attributes(0), &attributes);
}

#[test]
fn logical_ranges_are_whole_lines_and_invalid_ranges_do_not_mutate() {
    let mut e = TextEdit::new("abc\ndef");
    let before = e.clone();
    assert!(e.set_logical_block(1..2, BlockKind::Code, code()).is_err());
    assert_eq!(e, before);
    assert!(e.set_logical_block(0..4, BlockKind::Code, code()).is_err());
    assert_eq!(e, before);
    assert!(e.set_logical_block(0..7, BlockKind::Code, code()).is_ok());
    assert_eq!(e.logical_blocks().count(), 1);
}

#[test]
fn separate_legacy_pre_elements_are_distinct_and_logical_code_keeps_blank_lines() {
    let e = TextEdit::from_html("<pre><code>same</code></pre><pre><code>same</code></pre>");
    let blocks = e.logical_blocks().collect::<Vec<_>>();
    assert_eq!(blocks.len(), 2);
    assert_ne!(blocks[0].id, blocks[1].id);
    let restored = TextEdit::from_html(&e.to_html());
    assert_eq!(restored.logical_blocks().collect::<Vec<_>>(), blocks);

    let mut e = TextEdit::new("\n  body\t\n\n").rich();
    e.set_logical_block(0..e.text.len(), BlockKind::Code, code()).unwrap();
    let restored = TextEdit::from_html(&e.to_html());
    assert_eq!(restored.text, e.text);
    assert_eq!(restored.logical_blocks().collect::<Vec<_>>(), e.logical_blocks().collect::<Vec<_>>());
}

#[test]
fn unwrapping_a_logical_code_block_creates_distinct_paragraphs() {
    let mut e = TextEdit::new("a\nb\nc").rich();
    let id = e.set_logical_block(0..5, BlockKind::Code, code()).unwrap();
    e.set_selection(0, 5);
    e.set_block(Block::Paragraph);
    let blocks = e.logical_blocks().collect::<Vec<_>>();
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].id, id);
    assert_ne!(blocks[0].id, blocks[1].id);
    assert_ne!(blocks[1].id, blocks[2].id);
    e.undo();
    assert_eq!(e.logical_blocks().count(), 1);
    assert_eq!(e.logical_blocks().next().unwrap().id, id);
}
