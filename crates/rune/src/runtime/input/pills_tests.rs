use super::*;
use crate::{Key, Mods, TextEdit, TextStyle, text_input};
use crate::runtime::editor_selection_tests::State;

fn view(s: &State, cx: &mut Cx<State>) -> El {
    let mut root = crate::el().w_full().h_full();
    if s.pill_field_visible {
        let mut field = text_input(&s.edit, TextStyle::new(16.0, 400.0)).key("editor")
            .w_full().padding(8.0).on_edit(|s: &mut State| &mut s.edit)
            .on_submit(|s: &mut State, _| s.changes += 1)
            .on_input_key(|s: &mut State, ev, _| {
                if s.consume_pill_key && matches!(&ev.key, Key::Enter | Key::Text(_)) {
                    s.presses += 1;
                    true
                } else { false }
            });
        if s.observe_pills {
            field = field.on_input_pill_press(|s: &mut State, id, range, _| {
                s.pill_presses.push((id.to_owned(), range));
            });
        }
        if s.pill_single_line { field = field.single_line(); }
        if s.pill_normalize {
            field = field.on_input_selection_change(|s: &mut State, _| {
                if !s.edit.text.starts_with("**") {
                    s.edit.transact(crate::TransactionOptions::Transient, |e| { e.replace_range(0..0, "**"); }).unwrap();
                }
            });
        }
        root = root.child(crate::col().w_full().h_full()
            .scroll(crate::ScrollSpec { stick_bottom: false }).child(field));
    }
    if let Some(r) = cx.input_pill_rect("editor", "cite") {
        root = root.child(crate::el().key("pill-probe").size(r[2], r[3]).pin(Some(r[0]), Some(r[1]), None, None));
    }
    if let Some((_, range)) = s.pill_presses.last()
        && let Some(r) = cx.input_pill_range_rect("editor", "cite", range.clone())
    {
        root = root.child(crate::el().key("range-probe").size(r[2], r[3]).pin(Some(r[0]), Some(r[1]), None, None));
    }
    root
}

fn runtime(observe: bool) -> Runtime<State> {
    let mut edit = TextEdit::new("before citation after\nanother line\nanother line\nanother line\nlast").rich();
    edit.set_marks(vec![crate::MarkRange::new(7..15, crate::Mark::Pill("cite".into()))]);
    edit.set_caret(0);
    edit.focused = true;
    let mut rt = Runtime::new(State { edit, observe_pills: observe, ..Default::default() },
        view, (400.0, 100.0), Env::new(true, 2.0));
    rt.frame(0.0);
    rt
}

fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    let n = rt.nodes.iter().find(|n| n.key.as_deref() == Some(key)).unwrap();
    [n.target[0], n.target[1], n.local.w, n.local.h]
}

fn point(rt: &Runtime<State>) -> (f32, f32) {
    let r = rect(rt, "pill-probe");
    (r[0] + r[2] * 0.5, r[1] + r[3] * 0.5)
}

fn click(rt: &mut Runtime<State>) {
    let (x, y) = point(rt);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
}

fn key(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(0.0);
}

#[test]
fn a_pill_click_reports_its_id_and_model_range_without_changing_caret_placement() {
    let mut observed = runtime(true);
    let mut plain = runtime(false);
    click(&mut observed);
    click(&mut plain);
    assert_eq!(observed.state.pill_presses, vec![("cite".into(), 7..15)]);
    assert_eq!(observed.state.edit.selection(), plain.state.edit.selection());
    assert_eq!(observed.state.edit.text, plain.state.edit.text);
    assert_eq!(bytemuck::cast_slice::<_, u8>(&observed.instances), bytemuck::cast_slice::<_, u8>(&plain.instances));
    assert_eq!(rect(&observed, "range-probe"), rect(&observed, "pill-probe"));
}

#[test]
fn a_drag_across_a_pill_and_back_never_activates_it() {
    let mut rt = runtime(true);
    let (x, y) = point(&rt);
    rt.pointer_down(x, y);
    rt.pointer_move(x + 80.0, y);
    rt.frame(0.0);
    rt.pointer_move(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
    assert!(rt.state.pill_presses.is_empty());
}

#[test]
fn a_double_click_keeps_word_selection_and_activates_at_most_once() {
    let mut rt = runtime(true);
    let mut plain = runtime(false);
    click(&mut rt);
    click(&mut rt);
    click(&mut plain);
    click(&mut plain);
    assert_eq!(rt.state.edit.selection(), plain.state.edit.selection());
    assert_eq!(rt.state.edit.range(), (7, 15));
    assert_eq!(rt.state.pill_presses.len(), 1);
}

#[test]
fn pill_geometry_tracks_rewrapping_and_scrolling_as_a_view_dependency() {
    let mut rt = runtime(true);
    let before = rect(&rt, "pill-probe");
    rt.update(|s, tx| {
        tx.instant();
        s.edit.replace_range(0..0, "More prefix words that wrap before the citation. ");
    });
    rt.frame(0.0);
    rt.set_viewport((180.0, 100.0), 2.0);
    rt.frame(0.0);
    let wrapped = rect(&rt, "pill-probe");
    assert!(wrapped[1] > before[1]);
    rt.wheel(50.0, 50.0, -35.0);
    for _ in 0..10 { rt.frame(1.0 / 60.0); }
    let scrolled = rect(&rt, "pill-probe");
    assert!((wrapped[1] - scrolled[1] - 35.0).abs() < 1.0);
}

#[test]
fn enter_and_space_activate_exactly_selected_pills_without_editing_or_submitting() {
    let mut rt = runtime(true);
    rt.update(|s, _| s.edit.set_selection(15, 7));
    rt.frame(0.0);
    let before = rt.state.edit.text.clone();
    key(&mut rt, Key::Enter);
    key(&mut rt, Key::Text(" ".into()));
    assert_eq!(rt.state.pill_presses, vec![("cite".into(), 7..15); 2]);
    assert_eq!(rt.state.edit.text, before);
    assert_eq!(rt.state.edit.range(), (7, 15));
    assert_eq!(rt.state.changes, 0);
}

#[test]
fn input_and_app_key_handlers_can_consume_activation_first_and_repeats_do_not_fire() {
    let mut rt = runtime(true);
    rt.update(|s, _| { s.edit.set_selection(7, 15); s.consume_pill_key = true; });
    rt.frame(0.0);
    key(&mut rt, Key::Enter);
    assert_eq!(rt.state.presses, 1);
    assert!(rt.state.pill_presses.is_empty());
    rt.update(|s, _| s.consume_pill_key = false);
    rt.frame(0.0);
    rt.on_key = Some(|s, _, _| { s.presses += 1; true });
    key(&mut rt, Key::Text(" ".into()));
    assert_eq!(rt.state.presses, 2);
    assert!(rt.state.pill_presses.is_empty());
    rt.on_key = None;
    key(&mut rt, Key::Enter);
    rt.repeat_key(KeyEvent { key: Key::Enter, mods: Mods::default() });
    rt.frame(0.0);
    assert_eq!(rt.state.pill_presses.len(), 1);
}

#[test]
fn without_a_callback_space_keeps_the_editors_existing_pill_replacement() {
    let mut rt = runtime(false);
    rt.update(|s, _| s.edit.set_selection(7, 15));
    rt.frame(0.0);
    let mut expected = rt.state.edit.clone();
    expected.key(&KeyEvent { key: Key::Text(" ".into()), mods: Mods::default() }, None);
    key(&mut rt, Key::Text(" ".into()));
    assert_eq!(rt.state.edit.text, expected.text);
    assert_eq!(rt.state.edit.selection(), expected.selection());
    assert!(rt.state.pill_presses.is_empty());
}

#[test]
fn removing_or_securing_the_field_during_a_press_drops_activation_and_geometry() {
    for secure in [false, true] {
        let mut rt = runtime(true);
        let (x, y) = point(&rt);
        rt.pointer_down(x, y);
        rt.frame(0.0);
        rt.update(move |s, _| {
            if secure { s.edit.set_secure(true); } else { s.pill_field_visible = false; }
        });
        rt.frame(0.0);
        rt.pointer_up(x, y);
        rt.frame(0.0);
        assert!(rt.state.pill_presses.is_empty());
        assert!(rt.nodes.iter().enumerate().all(|(i, n)| n.key.as_deref() != Some("pill-probe") || rt.subtree_exiting(i)));
        assert_eq!(rt.input_pill_layout_rect(&rt.nodes, None, &rect_key("editor", "cite", None), true), None);
    }
}

#[test]
fn replacing_the_model_or_deleting_a_pill_between_down_and_up_does_not_activate() {
    for replace in [false, true] {
        let mut rt = runtime(true);
        let (x, y) = point(&rt);
        rt.pointer_down(x, y);
        rt.frame(0.0);
        rt.update(move |s, _| {
            if replace {
                let mut next = TextEdit::new(s.edit.text.clone()).rich();
                next.set_marks(s.edit.marks().to_vec());
                next.focused = true;
                s.edit = next;
            } else { s.edit.set_marks(Vec::new()); }
        });
        rt.frame(0.0);
        rt.pointer_up(x, y);
        rt.frame(0.0);
        assert!(rt.state.pill_presses.is_empty());
    }
}

#[test]
fn a_click_after_typing_still_activates_when_caret_placement_ends_the_history_run() {
    let mut rt = runtime(true);
    rt.update(|s, _| { s.edit.set_caret(s.edit.text.len()); s.edit.insert("!"); s.edit.set_caret(0); });
    rt.frame(0.0);
    click(&mut rt);
    assert_eq!(rt.state.pill_presses, vec![("cite".into(), 7..15)]);
}

#[test]
fn source_normalization_between_press_and_release_keeps_the_occurrence_and_updates_its_range() {
    let mut rt = runtime(true);
    rt.update(|s, _| s.pill_normalize = true);
    rt.frame(0.0);
    let (x, y) = point(&rt);
    rt.pointer_down(x, y);
    rt.frame(0.0);
    assert!(rt.state.edit.text.starts_with("**"));
    rt.pointer_up(x, y);
    rt.frame(0.0);
    assert_eq!(rt.state.pill_presses, vec![("cite".into(), 9..17)]);
    assert_eq!(rect(&rt, "range-probe"), rect(&rt, "pill-probe"));
}

#[test]
fn repeated_pill_ids_report_and_anchor_the_clicked_occurrence() {
    let mut rt = runtime(true);
    rt.update(|s, _| {
        s.edit = TextEdit::new("before citation after citation\nlast").rich();
        s.edit.set_marks(vec![
            crate::MarkRange::new(7..15, crate::Mark::Pill("cite".into())),
            crate::MarkRange::new(22..30, crate::Mark::Pill("cite".into())),
        ]);
        s.edit.focused = true;
        s.edit.set_caret(0);
        s.pill_presses = vec![("cite".into(), 22..30)];
    });
    rt.frame(0.0);
    let second = rect(&rt, "range-probe");
    assert!(second[0] > rect(&rt, "pill-probe")[0]);
    rt.update(|s, _| s.pill_presses.clear());
    rt.frame(0.0);
    let (x, y) = (second[0] + second[2] * 0.5, second[1] + second[3] * 0.5);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
    assert_eq!(rt.state.pill_presses, vec![("cite".into(), 22..30)]);
    assert_eq!(rect(&rt, "range-probe"), second);
}

#[test]
fn a_selection_larger_than_one_pill_keeps_ordinary_keyboard_editing() {
    let mut rt = runtime(true);
    rt.update(|s, _| s.edit.set_selection(0, 15));
    rt.frame(0.0);
    let mut expected = rt.state.edit.clone();
    expected.key(&KeyEvent { key: Key::Text(" ".into()), mods: Mods::default() }, None);
    key(&mut rt, Key::Text(" ".into()));
    assert_eq!(rt.state.edit.text, expected.text);
    assert!(rt.state.pill_presses.is_empty());
}

#[test]
fn declining_app_key_handlers_can_remove_rebind_or_deselect_the_field_safely() {
    for action in 0..3 {
        let mut rt = runtime(true);
        rt.update(|s, _| s.edit.set_selection(7, 15));
        rt.frame(0.0);
        rt.state.presses = action;
        rt.on_key = Some(|s, _, _| {
            if s.presses == 1 { s.pill_field_visible = false; }
            else if s.presses == 2 { s.edit.set_caret(0); }
            else {
                let mut edit = TextEdit::new(s.edit.text.clone()).rich();
                edit.set_marks(s.edit.marks().to_vec());
                edit.set_selection(7, 15);
                edit.focused = true;
                s.edit = edit;
            }
            false
        });
        key(&mut rt, Key::Enter);
        assert!(rt.state.pill_presses.is_empty());
    }
}

#[test]
fn pill_geometry_and_activation_follow_single_line_horizontal_scroll() {
    let mut rt = runtime(true);
    let prefix = "a long prefix before the citation ".repeat(8);
    let range = prefix.len()..prefix.len() + 8;
    let marked = range.clone();
    rt.update(move |s, _| {
        s.pill_single_line = true;
        s.edit = TextEdit::new(format!("{prefix}citation")).rich();
        s.edit.set_marks(vec![crate::MarkRange::new(marked, crate::Mark::Pill("cite".into()))]);
        s.edit.set_caret(0);
        s.edit.focused = true;
    });
    rt.frame(0.0);
    let before = rect(&rt, "pill-probe");
    assert!(before[0] > 400.0);
    rt.update(|s, _| s.edit.set_caret(s.edit.text.len()));
    rt.frame(0.0);
    let after = rect(&rt, "pill-probe");
    assert!(after[0] < before[0]);
    assert!(after[0] >= 8.0 && after[0] + after[2] <= 400.0);
    click(&mut rt);
    assert_eq!(rt.state.pill_presses, vec![("cite".into(), range)]);
}

#[test]
fn rtl_and_mixed_direction_pills_hit_their_drawn_rectangles() {
    for text in ["לפני הפניה אחרי", "לפני citation אחרי"] {
        let mut rt = runtime(true);
        let start = text.find(' ').unwrap() + 1;
        let end = text.rfind(' ').unwrap();
        rt.update(move |s, _| {
            s.edit = TextEdit::new(text).rich();
            s.edit.set_marks(vec![crate::MarkRange::new(start..end, crate::Mark::Pill("cite".into()))]);
            s.edit.set_caret(0);
            s.edit.focused = true;
        });
        rt.frame(0.0);
        let (x, y) = point(&rt);
        let editor = rt.nodes.iter().position(|n| n.key.as_deref() == Some("editor")).unwrap();
        assert!(rt.input_pill_at(editor, x, y).is_some(), "{text:?}, {:?}", rect(&rt, "pill-probe"));
        assert_eq!(rt.primary_target(x, y).and_then(|i| rt.textbox_input(i)), Some(editor),
            "{text:?}, {:?}", rect(&rt, "pill-probe"));
        click(&mut rt);
        assert_eq!(rt.state.pill_presses, vec![("cite".into(), start..end)],
            "{text:?}, {:?}, {:?}, {:?}", rect(&rt, "pill-probe"), rt.state.edit.selection(), rt.focus.path());
    }
}
