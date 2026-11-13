use super::*;
use crate::{BlockKind, BlockAttributes, EditorSelection, Key, Mods, TextEdit, TextStyle, text_input};

#[derive(Default)]
struct State { edit: TextEdit, changes: usize }

fn view(s: &State, _: &mut Cx<State>) -> El {
    text_input(&s.edit, TextStyle::new(16.0, 400.0)).key("editor").w(300.0)
        .on_edit(|s: &mut State| &mut s.edit)
        .on_input_selection_change(|s: &mut State, _| s.changes += 1)
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
