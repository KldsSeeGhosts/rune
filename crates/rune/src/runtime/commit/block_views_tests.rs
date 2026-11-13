use super::*;
use crate::{BlockAttributes, BlockKind, CodeAttributes, InputBlockView, Key, Mods, TextEdit, TextStyle, col, text, text_input};
use super::editor_selection_tests::State;

fn state() -> State {
    let mut edit = TextEdit::new("Words before the code that can wrap when the viewport narrows.\ntask\nlet a = 1;\nlast line").rich();
    let task = edit.set_logical_block(edit.line_range(1), BlockKind::Task { level: 0, checked: false }, BlockAttributes::default()).unwrap();
    let code = edit.set_logical_block(edit.line_range(2), BlockKind::Code,
        BlockAttributes::default().with_code(CodeAttributes::new("```", "rust"))).unwrap();
    edit.set_caret(edit.line_range(2).start);
    edit.focused = true;
    edit.clear_history();
    State { edit, code, task, views: true, ..Default::default() }
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let mut input = text_input(&s.edit, TextStyle::new(16.0, 400.0)).key("editor").w_full().padding(8.0)
        .on_edit(|s: &mut State| &mut s.edit);
    if s.views {
        // Register in reverse document order deliberately.
        input = input.input_block_view(InputBlockView::header(s.code,
            text("language").key("header").w_full().h(s.header_height).role("button")
                .focusable(!s.opt_out).on_press(|s: &mut State| s.presses += 1)
                .on_key(|s: &mut State, ev, _| {
                    if ev.key == Key::Text("k".into()) { s.presses += 10; true } else { false }
                })));
        input = input.input_block_view(InputBlockView::leading(s.task,
            crate::el().key("task-control").size(20.0, 20.0).role("checkbox")
                .on_press(|s: &mut State| {
                    let block = s.edit.logical_blocks().find(|b| b.id == s.task).unwrap();
                    let BlockKind::Task { level, checked } = block.kind else { return };
                    s.edit.update_logical_block(block.id, BlockKind::Task { level, checked: !checked }, block.attributes);
                })));
    }
    if s.secure { input = input.secure(); }
    col().key("viewport").w_full().h_full().scroll(crate::ScrollSpec { stick_bottom: false }).child(input)
}

fn runtime() -> Runtime<State> {
    let mut rt = Runtime::new(state(), view, (360.0, 140.0), Env::new(true, 2.0));
    rt.frame(0.0);
    rt
}

fn node<'a>(rt: &'a Runtime<State>, key: &str) -> &'a Node {
    rt.nodes.iter().find(|n| n.key.as_deref() == Some(key)).unwrap()
}

fn key(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(0.0);
}

fn click(rt: &mut Runtime<State>, name: &str) {
    let (x, y) = rt.find(name).unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
}

#[test]
fn absent_block_views_keep_the_plain_inputs_layout_measurement_and_paint() {
    let mut a = state();
    a.views = false;
    let mut b = state();
    b.code = TextEdit::new("").logical_blocks().next().unwrap().id;
    b.task = b.code;
    let mut plain = Runtime::new(a, view, (360.0, 140.0), Env::new(true, 2.0));
    let mut absent = Runtime::new(b, view, (360.0, 140.0), Env::new(true, 2.0));
    plain.frame(0.0);
    absent.frame(0.0);
    let geometry = |rt: &Runtime<State>| rt.nodes.iter().map(|n| {
        (n.path.clone(), n.target, n.text_layout.as_ref().map(|l| l.lines().map(|line| {
            let m = line.metrics();
            (line.text_range(), [m.inline_min_coord, m.offset, m.baseline, m.advance, m.block_max_coord])
        }).collect::<Vec<_>>()))
    }).collect::<Vec<_>>();
    assert_eq!(geometry(&plain), geometry(&absent));
    assert_eq!(bytemuck::cast_slice::<_, u8>(&plain.instances), bytemuck::cast_slice::<_, u8>(&absent.instances));
}

#[test]
fn block_header_press_does_not_move_the_caret_and_routes_the_press() {
    let mut rt = runtime();
    let before = rt.state.edit.selection();
    click(&mut rt, "header");
    assert_eq!(rt.state.presses, 1);
    assert_eq!(rt.state.edit.selection(), before);
    assert!(rt.focus.is(&node(&rt, "editor").path));
}

#[test]
fn block_header_height_pushes_text_down_and_updates_measurement() {
    let mut rt = runtime();
    let old = node(&rt, "editor").local.h;
    let header = node(&rt, "header").target;
    let e = node(&rt, "editor");
    let byte = rt.state.edit.logical_blocks().find(|b| b.id == rt.state.code).unwrap().range.start;
    let line = e.text_layout.as_ref().unwrap().lines().find(|l| l.text_range().start == byte).unwrap();
    assert!((e.target[1] + e.inset[1] + line.metrics().block_min_coord / 2.0 - header[3]).abs() < 0.1);
    rt.update(|s, tx| { tx.instant(); s.header_height += 40.0; });
    rt.frame(0.0);
    assert!((node(&rt, "editor").local.h - old - 40.0).abs() < 0.1);
}

#[test]
fn block_header_tracks_wrapping_on_resize_and_edits_above_it() {
    let mut rt = runtime();
    let old = node(&rt, "header").target[1];
    rt.set_viewport((180.0, 140.0), 2.0);
    rt.frame(0.0);
    let narrow = node(&rt, "header").target[1];
    assert!(narrow > old);
    let p = node(&rt, "editor");
    let content_width = p.local.w - p.inset[0] - p.inset[2];
    assert!((node(&rt, "header").local.w - content_width).abs() < 0.1);
    rt.update(|s, tx| { tx.instant(); s.edit.replace_range(0..0, "More words to wrap. "); });
    rt.frame(0.0);
    assert!(node(&rt, "header").target[1] > narrow);
}

#[test]
fn a_wrapping_header_remeasures_its_slot_at_the_new_block_width() {
    fn wrapping(s: &State, cx: &mut Cx<State>) -> El {
        let mut tree = view(s, cx);
        let field = tree.children.remove(0).input_block_view(InputBlockView::header(s.code,
            text("A language header with enough words to wrap when the input becomes narrow.")
                .key("header").w_full().wrap()));
        tree.child(field)
    }
    let mut rt = Runtime::new(state(), wrapping, (360.0, 140.0), Env::new(true, 2.0));
    rt.frame(0.0);
    let old = node(&rt, "header").local.h;
    rt.set_viewport((180.0, 140.0), 2.0);
    rt.frame(0.0);
    assert!(node(&rt, "header").local.h > old);
    let p = node(&rt, "editor");
    let byte = rt.state.edit.logical_blocks().find(|b| b.id == rt.state.code).unwrap().range.start;
    let line = p.text_layout.as_ref().unwrap().lines().find(|l| l.text_range().start == byte).unwrap();
    assert!((p.target[1] + p.inset[1] + line.metrics().block_min_coord / 2.0 - node(&rt, "header").target[3]).abs() < 0.1);
}

#[test]
fn block_header_scrolls_and_clips_with_its_editor() {
    let mut rt = runtime();
    let before = rt.presented("header").unwrap().0[1];
    rt.wheel(60.0, 60.0, -100.0);
    for _ in 0..30 { rt.frame(1.0 / 60.0); }
    let header = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("header")).unwrap();
    assert!(header.rect[1] < before);
    assert!(header.visible[1] >= 0.0);
    let e = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("editor")).unwrap();
    assert!((header.rect[1] - e.rect[1] - (node(&rt, "header").target[1] - node(&rt, "editor").target[1])).abs() < 1.0);
}

#[test]
fn caret_selection_and_copy_cross_a_header_without_including_it() {
    let mut rt = runtime();
    let end = rt.state.edit.text.len();
    rt.update(move |s, _| s.edit.set_selection(0, end));
    rt.frame(0.0);
    assert_eq!(rt.state.edit.selected(), rt.state.edit.text);
    assert!(!rt.state.edit.selected().contains("language"));
    let l = node(&rt, "editor").text_layout.as_ref().unwrap();
    let boxes = crate::text::range_boxes(l, 0, end, true);
    let h = node(&rt, "header").target;
    let origin = node(&rt, "editor").target[1] + node(&rt, "editor").inset[1];
    assert!(boxes.iter().all(|(r, _)| origin + r.y1 as f32 / 2.0 <= h[1] + 0.1 || origin + r.y0 as f32 / 2.0 >= h[3] - 0.1));
    rt.update(|s, _| { let at = s.edit.logical_blocks().find(|b| b.id == s.code).unwrap().range.start; s.edit.set_caret(at); });
    rt.frame(0.0);
    let at = rt.state.edit.focus;
    key(&mut rt, Key::Left, Mods::default());
    assert_eq!(rt.state.edit.focus, at - 1);
    key(&mut rt, Key::Right, Mods::default());
    assert_eq!(rt.state.edit.focus, at);
}

#[test]
fn block_header_preserves_composition_through_press_resize_and_commit() {
    let mut rt = runtime();
    let original = rt.state.edit.text.clone();
    rt.ime(Ime::Preedit("日本".into(), Some((3, 6))));
    rt.frame(0.0);
    let area = rt.ime_area;
    assert!(area.is_some());
    click(&mut rt, "header");
    assert!(rt.state.edit.is_composing());
    assert_eq!(rt.ime_area, area);
    assert_eq!(rt.state.edit.text, original);
    rt.set_viewport((240.0, 140.0), 2.0);
    rt.frame(0.0);
    assert!(rt.state.edit.is_composing());
    rt.ime(Ime::Commit("日本".into()));
    rt.frame(0.0);
    assert!(rt.state.edit.text.contains("日本"));
    assert_eq!(rt.state.edit.logical_blocks().find(|b| b.id == rt.state.code).unwrap().kind, BlockKind::Code);
}

#[test]
fn block_views_follow_identity_through_history_and_drop_missing_blocks() {
    let mut rt = runtime();
    let id = node(&rt, "header").id;
    rt.update(|s, _| s.edit.replace_range(0..0, "prefix "));
    rt.frame(0.0);
    assert_eq!(node(&rt, "header").id, id);
    key(&mut rt, Key::Command('z'), Mods { cmd: true, ..Default::default() });
    assert_eq!(node(&rt, "header").id, id);
    key(&mut rt, Key::Command('z'), Mods { cmd: true, shift: true, ..Default::default() });
    assert_eq!(node(&rt, "header").id, id);
    rt.update(|s, _| {
        let r = s.edit.logical_blocks().find(|b| b.id == s.code).unwrap().range;
        s.edit.replace_range(r.start.saturating_sub(1)..r.end, "");
    });
    rt.frame(0.0);
    assert!(rt.find("header").is_none());
    assert!(rt.state.edit.logical_blocks().all(|b| b.id != rt.state.code));
    key(&mut rt, Key::Command('z'), Mods { cmd: true, ..Default::default() });
    assert!(rt.find("header").is_some());
}

#[test]
fn block_views_tab_in_document_order_and_escape_restores_the_editor_caret() {
    let mut rt = runtime();
    let selection = rt.state.edit.selection();
    key(&mut rt, Key::Tab, Mods::default());
    assert!(rt.focus.is(&node(&rt, "task-control").path));
    key(&mut rt, Key::Tab, Mods::default());
    assert!(rt.focus.is(&node(&rt, "header").path));
    key(&mut rt, Key::Text(" ".into()), Mods::default());
    assert_eq!(rt.state.presses, 1);
    key(&mut rt, Key::Escape, Mods::default());
    assert!(rt.focus.is(&node(&rt, "editor").path));
    assert_eq!(rt.state.edit.selection(), selection);
}

#[test]
fn a_block_view_can_opt_out_of_focus_and_leading_press_keeps_the_caret() {
    let mut rt = runtime();
    rt.update(|s, _| s.opt_out = true);
    rt.frame(0.0);
    let selection = rt.state.edit.selection();
    click(&mut rt, "task-control");
    assert!(matches!(rt.state.edit.block_kind(1), BlockKind::Task { checked: true, .. }));
    assert_eq!(rt.state.edit.selection(), selection);
    key(&mut rt, Key::Tab, Mods::default());
    assert!(rt.focus.is(&node(&rt, "task-control").path));
    key(&mut rt, Key::Tab, Mods::default());
    assert!(rt.focus.is(&node(&rt, "editor").path));
}

#[test]
fn block_views_expose_ordinary_accessible_controls_and_actions() {
    let mut rt = runtime();
    let tree = rt.accessibility_tree();
    let id = crate::accesskit::NodeId(node(&rt, "header").id.0 << 16);
    let button = tree.nodes.iter().find(|(n, _)| *n == id).unwrap();
    assert_eq!(button.1.role(), crate::accesskit::Role::Button);
    assert!(rt.a11y_action(&crate::accesskit::ActionRequest {
        action: crate::accesskit::Action::Click, target_node: id,
        target_tree: crate::accesskit::TreeId::ROOT, data: None,
    }));
    rt.frame(0.0);
    assert_eq!(rt.state.presses, 1);
}

#[test]
fn block_view_key_handlers_receive_keys_without_editing_the_document() {
    let mut rt = runtime();
    let before = rt.state.edit.text.clone();
    key(&mut rt, Key::Tab, Mods::default());
    key(&mut rt, Key::Tab, Mods::default());
    key(&mut rt, Key::Text("k".into()), Mods::default());
    assert_eq!(rt.state.presses, 10);
    assert_eq!(rt.state.edit.text, before);
}

#[test]
fn removing_a_focused_block_view_returns_focus_to_the_editor() {
    let mut rt = runtime();
    key(&mut rt, Key::Tab, Mods::default());
    key(&mut rt, Key::Tab, Mods::default());
    rt.update(|s, _| {
        let r = s.edit.logical_blocks().find(|b| b.id == s.code).unwrap().range;
        s.edit.replace_range(r.start - 1..r.end, "");
    });
    rt.frame(0.0);
    assert!(rt.find("header").is_none());
    assert!(rt.focus.is(&node(&rt, "editor").path));
}

#[test]
fn secure_and_single_line_builders_drop_previously_attached_views() {
    let s = state();
    let build = || text_input(&s.edit, TextStyle::new(16.0, 400.0))
        .input_block_view(InputBlockView::header(s.code, text("language")));
    for field in [build().secure(), build().single_line()] {
        assert!(field.children.is_empty());
        assert!(field.cold.input.as_ref().unwrap().block_views.is_empty());
    }
}

#[test]
fn a_tall_leading_view_reserves_its_width_and_height() {
    fn tall(s: &State, cx: &mut Cx<State>) -> El {
        let mut tree = view(s, cx);
        let field = tree.children.remove(0).input_block_view(InputBlockView::leading(s.task,
            crate::el().key("task-control").size(50.0, 70.0)));
        tree.child(field)
    }
    let mut rt = Runtime::new(state(), tall, (360.0, 240.0), Env::new(true, 2.0));
    rt.frame(0.0);
    let leading = node(&rt, "task-control").target;
    assert!(leading[3] <= node(&rt, "header").target[1] + 0.1);
    let p = node(&rt, "editor");
    let byte = rt.state.edit.line_range(1).start;
    let line = p.text_layout.as_ref().unwrap().lines().find(|l| l.text_range().start == byte).unwrap();
    assert!(line.metrics().inline_min_coord / 2.0 >= 50.0);
}

#[test]
fn a_leading_view_reserves_the_right_edge_in_a_right_to_left_input() {
    fn rtl(s: &State, cx: &mut Cx<State>) -> El {
        let mut tree = view(s, cx);
        let field = tree.children.remove(0).dir(crate::Direction::Rtl)
            .input_block_view(InputBlockView::leading(s.task,
                crate::el().key("task-control").size(50.0, 20.0)));
        tree.child(field)
    }
    let mut rt = Runtime::new(state(), rtl, (360.0, 240.0), Env::new(true, 2.0));
    rt.frame(0.0);
    let p = node(&rt, "editor");
    let byte = rt.state.edit.line_range(1).start;
    let line = p.text_layout.as_ref().unwrap().lines().find(|l| l.text_range().start == byte).unwrap();
    let m = line.metrics();
    let end = p.target[0] + p.inset[0] + (m.inline_min_coord + m.offset + m.advance) / 2.0;
    assert!(end <= node(&rt, "task-control").target[0] + 0.1);
}

#[test]
fn preedit_before_a_block_maps_its_view_to_the_displayed_paragraph() {
    let mut rt = runtime();
    rt.update(|s, _| s.edit.set_caret(0));
    rt.frame(0.0);
    rt.ime(Ime::Preedit("日本 ".repeat(20), Some((0, 3))));
    rt.frame(0.0);
    let p = node(&rt, "editor");
    let start = p.text.as_ref().unwrap().content.find("let a").unwrap();
    let line = p.text_layout.as_ref().unwrap().lines().find(|l| l.text_range().start == start).unwrap();
    assert!((p.target[1] + p.inset[1] + line.metrics().block_min_coord / 2.0 - node(&rt, "header").target[3]).abs() < 0.1);
    assert!(rt.state.edit.is_composing());
}

#[test]
fn a_block_hidden_by_a_composition_replacement_has_no_widget_until_it_returns() {
    let mut rt = runtime();
    rt.update(|s, _| {
        let start = s.edit.line_range(1).start;
        let end = s.edit.line_range(2).end;
        s.edit.set_selection(start, end);
    });
    rt.frame(0.0);
    rt.ime(Ime::Preedit("日本".into(), Some((0, 3))));
    rt.frame(0.0);
    assert!(rt.find("header").is_none());
    rt.ime(Ime::Preedit("".into(), None));
    rt.frame(0.0);
    assert!(rt.find("header").is_some());
}

#[test]
fn block_view_hit_testing_inherits_the_editors_transform() {
    fn scaled(s: &State, cx: &mut Cx<State>) -> El {
        let mut tree = view(s, cx);
        let field = tree.children.remove(0).scale(1.25).origin(0.0, 0.0);
        tree.child(field)
    }
    let mut rt = Runtime::new(state(), scaled, (360.0, 240.0), Env::new(true, 2.0));
    rt.frame(0.0);
    let width = rt.presented("header").unwrap().0[2];
    assert!((width / node(&rt, "header").local.w - 1.25).abs() < 0.01);
    let before = rt.state.edit.selection();
    click(&mut rt, "header");
    assert_eq!(rt.state.presses, 1);
    assert_eq!(rt.state.edit.selection(), before);
}

#[test]
fn an_empty_code_block_keeps_its_header_while_composing_and_committing() {
    let mut s = state();
    s.edit = TextEdit::new("").rich();
    s.code = s.edit.set_logical_block(0..0, BlockKind::Code, BlockAttributes::default()
        .with_code(CodeAttributes::new("```", "rust"))).unwrap();
    s.edit.focused = true;
    let mut rt = Runtime::new(s, view, (360.0, 240.0), Env::new(true, 2.0));
    rt.frame(0.0);
    assert!(rt.find("header").is_some());
    assert!(node(&rt, "editor").local.h >= rt.state.header_height + 36.0);
    rt.ime(Ime::Preedit("日本".into(), Some((0, 3))));
    rt.frame(0.0);
    assert!(rt.find("header").is_some());
    rt.ime(Ime::Commit("日本".into()));
    rt.frame(0.0);
    assert!(rt.find("header").is_some());
    assert_eq!(rt.state.edit.logical_blocks().next().unwrap().id, rt.state.code);
}

#[test]
fn a_header_does_not_spring_separately_from_reflowed_text() {
    let mut rt = runtime();
    rt.update(|s, _| s.edit.replace_range(0..0, "More words that rewrap the text above the header. "));
    rt.frame(0.0);
    let p = node(&rt, "editor");
    let byte = rt.state.edit.logical_blocks().find(|b| b.id == rt.state.code).unwrap().range.start;
    let line = p.text_layout.as_ref().unwrap().lines().find(|l| l.text_range().start == byte).unwrap();
    let editor = rt.presented("editor").unwrap().0;
    let header = rt.presented("header").unwrap().0;
    let text_top = editor[1] + p.inset[1] + line.metrics().block_min_coord / 2.0;
    assert!((text_top - header[1] - header[3]).abs() < 0.1);
}

#[test]
fn securing_an_input_discards_widget_exit_visuals_in_the_same_frame() {
    let mut rt = runtime();
    rt.update(|s, _| s.secure = true);
    rt.frame(0.0);
    assert!(rt.presented("header").is_none());
    assert!(rt.presented("task-control").is_none());
    assert!(rt.elements().iter().all(|e| e.text.as_deref() != Some("language")));
}

#[test]
fn a_text_field_in_a_block_view_preserves_the_editors_pointer_focus_and_composition() {
    fn field(s: &State, cx: &mut Cx<State>) -> El {
        let mut tree = view(s, cx);
        let control = text_input(&s.widget_edit, TextStyle::new(16.0, 400.0))
            .key("header").focusable(true).w_full().h(32.0)
            .on_edit(|s: &mut State| &mut s.widget_edit)
            .on_press(|s: &mut State| s.presses += 1);
        let input = tree.children.remove(0)
            .input_block_view(InputBlockView::header(s.code, col().w_full().child(control)));
        tree.child(input)
    }
    let mut rt = Runtime::new(state(), field, (360.0, 240.0), Env::new(true, 2.0));
    rt.frame(0.0);
    rt.ime(Ime::Preedit("日本".into(), Some((0, 3))));
    rt.frame(0.0);
    let selection = rt.state.edit.selection();
    let area = rt.ime_area;
    click(&mut rt, "header");
    assert!(rt.focus.is(&node(&rt, "editor").path));
    assert_eq!(rt.state.edit.preedit(), Some(("日本", Some((0, 3)))));
    assert_eq!(rt.state.edit.selection(), selection);
    assert_eq!(rt.ime_area, area);
    assert_eq!(rt.state.presses, 1);
}
