use super::*;
use crate::{BlockKind, BlockAttributes, EditorSelection, Key, Mods, TextEdit, TextStyle, text_input};

#[derive(Default)]
struct State { edit: TextEdit, changes: usize, style: crate::BlockStyle }

fn view(s: &State, _: &mut Cx<State>) -> El {
    let input = text_input(&s.edit, TextStyle::new(16.0, 400.0)).key("editor").w(300.0)
        .block_style(s.style)
        .on_edit(|s: &mut State| &mut s.edit)
        .on_input_selection_change(|s: &mut State, _| s.changes += 1);
    crate::col().items_start().child(input)
}

#[test]
fn rule_margins_are_intrinsic_height_and_atomic_pointer_targets() {
    let mut edit = TextEdit::new("above\n\nbelow").rich();
    let id = edit.set_logical_block(6..6, BlockKind::Rule, BlockAttributes::default()).unwrap();
    edit.focused = true;
    let mut rt = Runtime::new(State { edit, ..Default::default() }, view, (400.0, 300.0), Env::new(true, 2.0));
    rt.frame(0.0);
    let old = rt.nodes.iter().find(|n| n.key.as_deref() == Some("editor")).unwrap().local.h;
    rt.update(|s, tx| {
        tx.instant();
        s.style.rule_margin_top = Some(15.0);
        s.style.rule_margin_bottom = Some(25.0);
    });
    rt.frame(0.0);
    let n = rt.nodes.iter().find(|n| n.key.as_deref() == Some("editor")).unwrap();
    assert!((n.local.h - old - 40.0).abs() < 0.1, "old {old}, new {}, style {:?}, layout {} measured {}", n.local.h,
        n.text.as_ref().unwrap().cold.block_style,
        n.text_layout.as_ref().unwrap().height(),
        n.text.as_ref().unwrap().measured_height(n.text_layout.as_ref().unwrap(), 2.0));
    let line = n.text_layout.as_ref().unwrap().lines().nth(1).unwrap();
    let m = line.metrics();
    let ys = [n.target[1] + m.block_min_coord / 2.0 - 14.0, n.target[1] + m.block_max_coord / 2.0 + 24.0];
    for y in ys {
        rt.update(|s, _| s.edit.set_caret(0));
        rt.frame(0.0);
        rt.pointer_down(100.0, y);
        rt.pointer_up(100.0, y);
        rt.frame(0.0);
        assert_eq!(rt.state.edit.selection(), EditorSelection::Block(id));
    }
}

#[test]
fn checked_tasks_have_independent_fill_and_two_stroke_tick_colors() {
    let mut edit = TextEdit::new("task").rich();
    edit.set_block_kind(0, BlockKind::Task { level: 0, checked: true });
    let mut rt = Runtime::new(State { edit, ..Default::default() }, view, (400.0, 300.0), Env::new(true, 2.0));
    rt.frame(0.0);
    assert!(rt.instances.iter().all(|i| i.params[1] != KIND_SEGMENT));
    let rects: Vec<_> = rt.instances.iter().filter(|i| i.params[1] == KIND_RECT).collect();
    assert_eq!(rects.len(), 5);
    assert_eq!(rects[0].rect[2..], [24.0, 2.0]);
    assert!((rects[4].rect[2] - 14.4).abs() < 0.01);
    assert_eq!(rects[0].fill, rects[4].fill);
    let red = crate::Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    let green = crate::Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    rt.update(move |s, tx| {
        tx.instant();
        s.style.checkbox_checked_fill = Some(red);
    });
    rt.frame(0.0);
    assert!(rt.instances.iter().all(|i| i.params[1] != KIND_SEGMENT));
    assert_eq!(rt.instances.iter().filter(|i| i.params[1] == KIND_RECT).count(), 6);
    rt.update(move |s, tx| { tx.instant(); s.style.checkbox_mark_color = Some(green); });
    rt.frame(0.0);
    let ticks: Vec<_> = rt.instances.iter().filter(|i| i.params[1] == KIND_SEGMENT).collect();
    assert_eq!(ticks.len(), 2);
    assert!(ticks.iter().all(|i| i.fill == green.premul(1.0)));
    assert!(rt.instances.iter().any(|i| i.params[1] == KIND_RECT && i.fill == red.premul(1.0)));
    assert_eq!([ticks[0].radii[2], ticks[0].radii[3]], [ticks[1].radii[0], ticks[1].radii[1]]);
    assert!(ticks[0].radii[3] > ticks[0].radii[1]);
    assert!(ticks[1].radii[3] < ticks[1].radii[1]);
    rt.update(|s, tx| { tx.instant(); s.edit.set_task_checked(0, false); });
    rt.frame(0.0);
    assert!(rt.instances.iter().all(|i| i.params[1] != KIND_SEGMENT && i.fill != red.premul(1.0)));
}

#[test]
fn rule_hit_testing_accessibility_and_deletion_use_atomic_selection() {
    let mut edit = TextEdit::new("above\n\nbelow").rich();
    let id = edit.set_logical_block(6..6, BlockKind::Rule, BlockAttributes::default()).unwrap();
    edit.focused = true;
    let mut rt = Runtime::new(State { edit, ..Default::default() }, view, (400.0, 200.0), Env::new(true, 2.0));
    rt.frame(0.0);
    let n = rt.nodes.iter().find(|n| n.key.as_deref() == Some("editor")).unwrap();
    let laid = n.text_layout.as_ref().unwrap().lines().nth(1).unwrap();
    let line = laid.metrics();
    let y = n.target[1] + (line.block_min_coord + line.block_max_coord) * 0.25;
    rt.pointer_down(100.0, y);
    rt.pointer_up(100.0, y);
    rt.frame(0.0);
    assert_eq!(rt.state.edit.selection(), EditorSelection::Block(id));
    assert_eq!(rt.state.changes, 1);
    rt.key(KeyEvent { key: Key::Delete, mods: Mods::default() });
    rt.frame(0.0);
    assert_eq!(rt.state.edit.text, "above\nbelow");
    rt.key(KeyEvent { key: Key::Command('z'), mods: Mods { cmd: true, ..Default::default() } });
    rt.frame(0.0);
    assert_eq!(rt.state.edit.selection(), EditorSelection::Block(id));
    rt.update(|s, _| s.edit.set_caret(0));
    rt.frame(0.0);
    let tree = rt.accessibility_tree();
    let target = tree.nodes.iter().find(|(_, n)| n.role() == crate::accesskit::Role::Splitter).unwrap().0;
    assert!(rt.a11y_action(&crate::accesskit::ActionRequest {
        action: crate::accesskit::Action::Click, target_node: target,
        target_tree: crate::accesskit::TreeId::ROOT, data: None,
    }));
    rt.frame(0.0);
    assert_eq!(rt.state.edit.selection(), EditorSelection::Block(id));
    let tree = rt.accessibility_tree();
    assert!(tree.nodes.iter().any(|(_, n)| n.role() == crate::accesskit::Role::Splitter && n.is_selected() == Some(true)));
}

#[test]
fn clicking_the_default_task_checkbox_does_not_move_the_caret() {
    let mut edit = TextEdit::new("task\ncaret stays here").rich();
    edit.set_block_kind(0, BlockKind::Task { level: 0, checked: false });
    edit.focused = true;
    let caret = edit.focus;
    let mut rt = Runtime::new(State { edit, ..Default::default() }, view, (400.0, 200.0), Env::new(true, 2.0));
    rt.frame(0.0);
    let n = rt.nodes.iter().find(|n| n.key.as_deref() == Some("editor")).unwrap();
    let first = n.text_layout.as_ref().unwrap().lines().next().unwrap();
    let y = n.target[1] + first.metrics().baseline / 2.0 - 16.0 * 0.34;
    rt.pointer_down(n.target[0] + 12.0, y);
    rt.pointer_up(12.0, y);
    rt.frame(0.0);
    assert_eq!(rt.state.edit.block_kind(0), BlockKind::Task { level: 0, checked: true });
    assert_eq!((rt.state.edit.anchor, rt.state.edit.focus), (caret, caret));
    assert_eq!(rt.state.changes, 0);
    rt.key(KeyEvent { key: Key::Command('z'), mods: Mods { cmd: true, ..Default::default() } });
    rt.frame(0.0);
    assert_eq!(rt.state.edit.block_kind(0), BlockKind::Task { level: 0, checked: false });
}
