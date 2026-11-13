use super::*;

fn rule(text: &str, line: usize) -> (TextEdit, BlockId) {
    let mut e = TextEdit::new(text).rich();
    let r = e.line_range(line);
    let id = e.set_logical_block(r, BlockKind::Rule,
        BlockAttributes::default().with_rule_source("* * *")).unwrap();
    e.clear_history();
    (e, id)
}

fn key(e: &mut TextEdit, key: Key) {
    e.key(&KeyEvent { key, mods: Mods::default() }, None);
}

#[test]
fn a_rule_has_atomic_selection_deletion_and_history() {
    let (mut e, id) = rule("above\n\nbelow", 1);
    assert!(e.select_block(id));
    assert_eq!(e.selection(), EditorSelection::Block(id));
    assert_eq!(e.selected(), "* * *");
    key(&mut e, Key::Backspace);
    assert_eq!(e.text, "above\nbelow");
    assert!(matches!(e.selection(), EditorSelection::Text { .. }));
    assert_eq!(e.logical_blocks().count(), 2);
    e.undo();
    assert_eq!(e.text, "above\n\nbelow");
    assert_eq!(e.selection(), EditorSelection::Block(id));
    assert_eq!(e.block_attributes(1).rule_source(), Some("* * *"));
    e.redo();
    assert_eq!(e.text, "above\nbelow");
}

#[test]
fn typing_replaces_the_rule_in_its_slot_and_does_not_merge_its_neighbors() {
    let (mut e, id) = rule("above\n\nbelow", 1);
    e.select_block(id);
    key(&mut e, Key::Text("replacement".into()));
    assert_eq!(e.text, "above\nreplacement\nbelow");
    assert_eq!(e.block_kind(1), BlockKind::Paragraph);
    assert_eq!(e.block_kind(2), BlockKind::Paragraph);
    e.undo();
    assert_eq!(e.selection(), EditorSelection::Block(id));
}

#[test]
fn deleting_the_only_rule_and_the_last_rule_are_undoable() {
    for (text, line, remaining) in [("", 0, ""), ("above\n", 1, "above")] {
        let (mut e, id) = rule(text, line);
        e.select_block(id);
        key(&mut e, Key::Delete);
        assert_eq!(e.text, remaining);
        assert!(e.logical_blocks().all(|b| b.kind != BlockKind::Rule));
        e.undo();
        assert_eq!(e.selection(), EditorSelection::Block(id));
        e.redo();
        assert_eq!(e.text, remaining);
    }
}

#[test]
fn arrows_and_adjacent_backspace_navigate_to_the_rule_as_one_object() {
    let (mut e, id) = rule("a\n\nb", 1);
    e.set_caret(1);
    key(&mut e, Key::Right);
    assert_eq!(e.selection(), EditorSelection::Block(id));
    key(&mut e, Key::Right);
    assert_eq!(e.selection(), EditorSelection::Text { anchor: 3, focus: 3 });
    key(&mut e, Key::Backspace);
    assert_eq!(e.selection(), EditorSelection::Block(id));
    key(&mut e, Key::Left);
    assert_eq!(e.selection(), EditorSelection::Text { anchor: 1, focus: 1 });
}

#[test]
fn a_selected_rule_copies_as_rich_content_and_cross_block_selection_deletes_it() {
    let (mut e, id) = rule("a\n\nb", 1);
    e.select_block(id);
    let html = e.selected_html();
    let mut pasted = TextEdit::new("").rich();
    pasted.paste_html(&html, "");
    assert_eq!(pasted.block_kind(0), BlockKind::Rule);
    assert_ne!(pasted.logical_blocks().next().unwrap().id, id);
    e.set_selection(0, e.text.len());
    e.insert("");
    assert_eq!(e.text, "");
    assert_eq!(e.block_kind(0), BlockKind::Paragraph);
    e.undo();
    assert_eq!(e.block_kind(1), BlockKind::Rule);
}

#[test]
fn a_rule_cannot_keep_hidden_text_after_legacy_or_direct_edits() {
    let (mut e, id) = rule("", 0);
    e.set_caret(0);
    e.insert("text");
    assert_eq!(e.block_kind(0), BlockKind::Paragraph);
    e.undo();
    assert_eq!(e.block_kind(0), BlockKind::Rule);
    e.select_block(id);
    e.set_caret(0);
    assert_eq!(e.selection(), EditorSelection::Text { anchor: 0, focus: 0 });
    e.text.push_str("direct assignment");
    e.sync_invariants();
    assert_eq!(e.block_kind(0), BlockKind::Paragraph);
}

#[test]
fn deleting_a_last_rule_retains_the_empty_preceding_paragraphs_identity() {
    let (mut e, id) = rule("\n", 1);
    let previous = e.logical_blocks().next().unwrap().id;
    e.set_block_attributes(0, BlockAttributes::default().with_quote_prefix("> "));
    e.select_block(id);
    key(&mut e, Key::Delete);
    assert_eq!(e.text, "");
    assert_eq!(e.logical_blocks().next().unwrap().id, previous);
    assert_eq!(e.block_attributes(0).quote_prefix(), Some("> "));
}

#[test]
fn ime_commit_replaces_a_selected_rule_and_undo_restores_it() {
    let (mut e, id) = rule("", 0);
    e.select_block(id);
    e.ime(&Ime::Preedit("日本".into(), Some((3, 6))));
    assert_eq!(e.block_kind(0), BlockKind::Rule);
    e.ime(&Ime::Commit("日本".into()));
    assert_eq!(e.text, "日本");
    assert_eq!(e.block_kind(0), BlockKind::Paragraph);
    e.undo();
    assert_eq!(e.selection(), EditorSelection::Block(id));
}
