use super::*;

fn authored() -> BlockAttributes {
    BlockAttributes::default()
        .with_indent("\t  ")
        .with_marker("009)")
        .with_space("\t")
        .with_marker_space("  ")
        .with_content_space("")
        .with_quote_prefix(">\t")
        .with_heading_space("  ")
        .with_rule_source("* * *")
}

#[test]
fn authored_attributes_follow_edits_splits_joins_and_history() {
    let mut e = TextEdit::new("left\nright").rich();
    e.set_block_attributes(0, authored());
    e.clear_history();
    e.set_caret(2);
    e.insert("!\n");
    assert_eq!(e.text, "le!\nft\nright");
    assert_eq!(e.block_attributes(0), &authored());
    assert_eq!(e.block_attributes(1), &authored());
    assert_eq!(e.block_attributes(2), &BlockAttributes::default());
    e.undo();
    assert_eq!(e.text, "left\nright");
    assert_eq!(e.block_attributes(0), &authored());
    e.redo();
    e.set_selection(3, 4);
    e.insert("");
    assert_eq!(e.text, "le!ft\nright");
    assert_eq!(e.block_attributes(0), &authored());
    e.undo();
    assert_eq!(e.block_attributes(1), &authored());
    e.set_block_attributes(1, BlockAttributes::default().with_marker("+"));
    e.undo();
    assert_eq!(e.block_attributes(1), &authored());
    e.redo();
    assert_eq!(e.block_attributes(1).marker(), Some("+"));
}

#[test]
fn deleting_empty_prefix_keeps_right_hand_block_data() {
    let mut e = TextEdit::new("\nright").rich();
    e.set_block_kind(1, BlockKind::Heading { level: 3 });
    e.set_block_attributes(1, authored());
    e.set_selection(0, 1);
    e.insert("");
    assert_eq!(e.block_kind(0), BlockKind::Heading { level: 3 });
    assert_eq!(e.block_attributes(0), &authored());
}

#[test]
fn new_kinds_keep_legacy_block_copy_and_exhaustive_matches() {
    fn legacy(b: Block) -> Block {
        match b {
            Block::Paragraph => b,
            Block::Bullet(_) => b,
            Block::Numbered(_) => b,
            Block::Quote => b,
            Block::Code => b,
        }
    }
    let mut e = TextEdit::new("heading\ntask\n").rich();
    e.set_block_kind(0, BlockKind::Heading { level: 2 });
    e.set_block_kind(1, BlockKind::Task { level: 8, checked: true });
    e.set_block_kind(2, BlockKind::Rule);
    assert_eq!(legacy(e.blocks()[0]), Block::Paragraph);
    assert_eq!(e.blocks()[1], Block::Bullet(8));
    assert_eq!(e.block_kind(2), BlockKind::Rule);
    e.set_task_checked(1, false);
    assert_eq!(e.block_kind(1), BlockKind::Task { level: 8, checked: false });
    e.undo();
    assert_eq!(e.block_kind(1), BlockKind::Task { level: 8, checked: true });
    e.redo();
    e.set_blocks(vec![Block::Paragraph; 3]);
    assert_eq!(e.block_kind(1), BlockKind::Paragraph);
}

#[test]
fn task_enter_continues_unchecked_and_indent_keeps_authored_data() {
    let mut e = TextEdit::new("task").rich();
    e.set_block_kind(0, BlockKind::Task { level: 4, checked: true });
    e.set_block_attributes(0, authored());
    e.key(&KeyEvent { key: Key::Enter, mods: Mods::default() }, None);
    assert_eq!(e.text, "task\n");
    assert_eq!(e.block_kind(0), BlockKind::Task { level: 4, checked: true });
    assert_eq!(e.block_kind(1), BlockKind::Task { level: 4, checked: false });
    assert_eq!(e.block_attributes(1), &authored());
    assert!(e.indent_list());
    assert_eq!(e.block_kind(1), BlockKind::Task { level: 5, checked: false });
    assert!(e.outdent_list());
    assert_eq!(e.block_kind(1), BlockKind::Task { level: 4, checked: false });
    assert_eq!(e.block_attributes(1), &authored());
}

#[test]
fn heading_return_starts_a_paragraph_and_keeps_the_original_heading() {
    let mut e = TextEdit::new("Title").rich();
    e.set_block_kind(0, BlockKind::Heading { level: 2 });
    e.set_block_attributes(0, authored());
    e.clear_history();
    e.key(&KeyEvent { key: Key::Enter, mods: Mods::default() }, None);
    assert_eq!(e.text, "Title\n");
    assert_eq!(e.block_kind(0), BlockKind::Heading { level: 2 });
    assert_eq!(e.block_kind(1), BlockKind::Paragraph);
    e.undo();
    assert_eq!(e.text, "Title");
    assert_eq!(e.block_kind(0), BlockKind::Heading { level: 2 });
}

#[test]
fn attributed_html_and_rich_paste_round_trip_empty_and_nonempty_lines() {
    let mut e = TextEdit::new("Title\ntask\n\nquote\nordered").rich();
    let kinds = [
        BlockKind::Heading { level: 6 },
        BlockKind::Task { level: 12, checked: true },
        BlockKind::Rule,
        BlockKind::Quote,
        BlockKind::Numbered(15),
    ];
    for (i, kind) in kinds.into_iter().enumerate() {
        e.set_block_kind(i, kind);
        e.set_block_attributes(i, authored());
    }
    let html = e.to_html();
    let rt = TextEdit::from_html(&html);
    assert_eq!(rt.text, e.text, "{html}");
    for (i, kind) in kinds.into_iter().enumerate() {
        assert_eq!(rt.block_kind(i), kind, "{html}");
        assert_eq!(rt.block_attributes(i), &authored());
    }
    let mut pasted = TextEdit::new("").rich();
    pasted.paste_html(&html, &e.text);
    assert_eq!(pasted.text, e.text);
    for (i, kind) in kinds.into_iter().enumerate() {
        assert_eq!(pasted.block_kind(i), kind);
        assert_eq!(pasted.block_attributes(i), &authored());
    }
    pasted.undo();
    assert_eq!(pasted.text, "");
    pasted.redo();
    assert_eq!(pasted.block_kind(2), BlockKind::Rule);
}

#[test]
fn new_blocks_keep_bidi_and_composition_model_boundaries() {
    for kind in [
        BlockKind::Heading { level: 2 },
        BlockKind::Task { level: 5, checked: false },
    ] {
        let mut e = TextEdit::new("שלום mixed العربية").rich();
        e.set_block_kind(0, kind);
        e.set_block_attributes(0, authored());
        e.set_caret("שלום".len());
        let original = e.text.clone();
        e.ime(&Ime::Preedit("日本".into(), Some((3, 6))));
        assert_eq!(e.text, original);
        assert_eq!(e.block_kind(0), kind);
        assert!(e.display().0.contains("日本"));
        e.ime(&Ime::Commit("日本".into()));
        assert!(e.text.is_char_boundary(e.focus));
        assert_eq!(e.block_kind(0), kind);
        assert_eq!(e.block_attributes(0), &authored());
        e.undo();
        assert_eq!(e.text, original);
        e.redo();
        assert!(e.text.contains("日本"));
    }
}

#[test]
fn headings_use_app_sizes_and_tasks_and_rules_reach_layout() {
    use crate::{El, Env, Runtime, TextStyle, Cx};
    fn view(e: &TextEdit, _: &mut Cx<TextEdit>) -> El {
        crate::text_input(e, TextStyle::new(16.0, 400.0)).key("editor").w_full()
            .block_style(BlockStyle { heading_scale: [2.0; 6], heading_weight: [700.0; 6], ..Default::default() })
    }
    let mut e = TextEdit::new("שלום English العربية\ntask\n").rich();
    e.set_block_kind(0, BlockKind::Heading { level: 2 });
    e.set_block_kind(1, BlockKind::Task { level: 1, checked: true });
    e.set_block_kind(2, BlockKind::Rule);
    let mut rt = Runtime::new(e, view, (400.0, 300.0), Env::new(true, 2.0));
    rt.frame(0.0);
    let n = rt.nodes.iter().find(|n| n.key.as_deref() == Some("editor")).unwrap();
    let spec = n.text.as_ref().unwrap();
    assert!(spec.spans.iter().any(|s| s.size == Some(32.0) && s.weight == Some(700.0)));
    assert_eq!(spec.cold.block_kinds[1], BlockKind::Task { level: 1, checked: true });
    let layout = n.text_layout.as_ref().unwrap();
    assert!(layout.lines().count() >= 3);
    let ev = KeyEvent { key: Key::Left, mods: Mods::default() };
    rt.state.set_caret("שלום".len());
    rt.state.key(&ev, Some(layout));
    assert!(rt.state.text.is_char_boundary(rt.state.focus));
    let tree = rt.accessibility_tree();
    assert!(tree.nodes.iter().any(|(_, n)| n.role() == crate::accesskit::Role::Heading && n.level() == Some(2)));
    assert!(tree.nodes.iter().any(|(_, n)| n.role() == crate::accesskit::Role::CheckBox && n.toggled() == Some(crate::accesskit::Toggled::True)));
    assert!(tree.nodes.iter().any(|(_, n)| n.role() == crate::accesskit::Role::Splitter));
}
