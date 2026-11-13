use super::*;

#[test]
fn app_marks_have_independent_boundary_affinity_and_no_intrinsic_style() {
    for start in [false, true] {
        for end in [false, true] {
            let mut e = TextEdit::new("abc").rich();
            let mark = AppMark::new("source").inclusive(start, end);
            e.set_app_mark(0..3, mark.clone());
            e.set_caret(0);
            e.insert("<");
            e.set_caret(e.text.len());
            e.insert(">");
            let ranges = e.app_marks().collect::<Vec<_>>();
            assert_eq!(ranges, vec![(usize::from(!start)..if end { 5 } else { 4 }, &mark)]);
            let (spans, _, _, _) = e.rich_display_specs(crate::TextStyle::new(16.0, 400.0), RichStyle::default());
            assert!(spans.is_empty());
            e.undo();
            e.redo();
            assert_eq!(e.app_marks().next().unwrap().1, &mark);
        }
    }
}

#[test]
fn app_marks_keep_ids_through_code_edits_history_html_and_paste() {
    let mut e = TextEdit::new("a\nb").rich();
    e.set_logical_block(0..3, BlockKind::Code, BlockAttributes::default()).unwrap();
    let mark = AppMark::new("a<&\"").inclusive(false, false);
    e.set_app_mark(0..3, mark.clone());
    e.clear_history();
    e.replace_range(1..1, "x");
    assert_eq!(e.app_marks().next(), Some((0..4, &mark)));
    let restored = TextEdit::from_html(&e.to_html());
    assert_eq!(restored.app_marks().collect::<Vec<_>>(), e.app_marks().collect::<Vec<_>>());
    let mut pasted = TextEdit::new("").rich();
    pasted.paste_html(&e.to_html(), &e.text);
    assert_eq!(pasted.app_marks().collect::<Vec<_>>(), e.app_marks().collect::<Vec<_>>());
    e.undo();
    assert_eq!(e.app_marks().next(), Some((0..3, &mark)));
    e.redo();
    assert_eq!(e.app_marks().next(), Some((0..4, &mark)));
    e.remove_app_mark("a<&\"");
    assert_eq!(e.app_marks().count(), 0);
    e.undo();
    assert_eq!(e.app_marks().count(), 1);
}

#[test]
fn edit_maps_compose_and_invert_with_explicit_boundary_bias() {
    let mut e = TextEdit::new("ab");
    e.set_caret(1);
    let map = e.transact(TransactionOptions::Transient, |e| {
        e.replace_range(0..0, "**");
        e.replace_range(4..4, "**");
    }).unwrap();
    assert_eq!(e.text, "**ab**");
    assert_eq!((e.anchor, e.focus), (3, 3));
    assert_eq!(map.map(0, MapBias::Before), 0);
    assert_eq!(map.map(0, MapBias::After), 2);
    assert_eq!(map.map(2, MapBias::Before), 4);
    assert_eq!(map.map(2, MapBias::After), 6);
    assert_eq!(map.inverse().map(3, MapBias::After), 1);
    assert_eq!(map.then(&map.inverse()).map(1, MapBias::After), 1);
}

#[test]
fn source_reveal_after_undo_preserves_redo_and_rebases_the_next_recorded_edit() {
    let mut e = TextEdit::new("bold").rich();
    e.set_selection(0, 4);
    e.set_mark(Mark::Bold);
    e.clear_history();
    e.set_caret(4);
    e.insert("!");
    e.undo();
    assert_eq!(e.text, "bold");
    let reveal = |e: &mut TextEdit| {
        e.transact(TransactionOptions::Transient, |e| {
            e.replace_range(0..0, "**");
            e.replace_range(e.text.len()..e.text.len(), "**");
            e.set_marks(vec![]);
            e.set_app_mark(0..e.text.len(), AppMark::new("source"));
        }).unwrap()
    };
    reveal(&mut e);
    assert_eq!(e.text, "**bold**");
    assert!(!e.history.redo.is_empty());
    e.redo();
    assert_eq!(e.text, "bold!");
    e.undo();
    reveal(&mut e);
    e.set_caret(4);
    e.insert("X");
    assert_eq!(e.text, "**boXld**");
    e.undo();
    assert_eq!(e.text, "bold");
    assert!(e.has_mark(&Mark::Bold));
    e.redo();
    assert_eq!(e.text, "**boXld**");
}

#[test]
fn transient_collapse_and_reveal_keep_history_and_selections_in_their_own_shapes() {
    let mut e = TextEdit::new("hello").rich();
    e.set_caret(5);
    e.insert("!");
    e.undo();
    e.set_caret(2);
    e.transact(TransactionOptions::Transient, |e| {
        e.replace_range(0..0, "*");
        e.replace_range(e.text.len()..e.text.len(), "*");
    }).unwrap();
    assert_eq!(e.focus, 3);
    e.transact(TransactionOptions::Transient, |e| {
        e.replace_range(e.text.len() - 1..e.text.len(), "");
        e.replace_range(0..1, "");
    }).unwrap();
    assert_eq!(e.text, "hello");
    assert_eq!(e.focus, 2);
    e.redo();
    assert_eq!(e.text, "hello!");
}

#[test]
fn recorded_transactions_are_one_step_and_transient_rewrites_refuse_composition() {
    let mut e = TextEdit::new("ab");
    e.transact(TransactionOptions::Recorded, |e| {
        e.replace_range(0..0, "<");
        e.replace_range(e.text.len()..e.text.len(), ">");
    }).unwrap();
    e.undo();
    assert_eq!(e.text, "ab");
    e.redo();
    assert_eq!(e.text, "<ab>");
    e.ime(&Ime::Preedit("界".into(), None));
    let before = e.clone();
    assert!(e.transact(TransactionOptions::Transient, |e| e.insert("bad")).is_err());
    assert_eq!(e, before);
}

#[test]
fn app_marks_with_different_ids_do_not_merge_and_survive_unrelated_formatting() {
    let mut e = TextEdit::new("abcdef").rich().allow_marks([Mark::Bold]);
    e.set_app_mark(0..3, AppMark::new("a"));
    e.set_app_mark(2..6, AppMark::new("b").inclusive(true, true));
    e.set_selection(0, 6);
    e.set_mark(Mark::Bold);
    assert_eq!(e.app_marks().count(), 2);
    let r = TextEdit::from_html(&e.to_html());
    assert_eq!(r.app_marks().collect::<Vec<_>>(), e.app_marks().collect::<Vec<_>>());
}

#[test]
fn a_panicking_transaction_rolls_back_document_history_and_detached_source() {
    let mut e = TextEdit::new("ab");
    e.insert("X");
    e.undo();
    e.transact(TransactionOptions::Transient, |e| e.replace_range(0..0, "*")).unwrap();
    let before = e.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = e.transact(TransactionOptions::Recorded, |e| {
            e.insert("bad");
            panic!("fixture");
        });
    }));
    assert!(result.is_err());
    assert_eq!(e, before);
    e.redo();
    assert_eq!(e.text, "abX");
}
