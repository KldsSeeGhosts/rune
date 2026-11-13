use super::*;
use crate::{Key, Mods, TextEdit, TextStyle, text_input};

#[derive(Default)]
struct State {
    edit: TextEdit,
    selections: Vec<(usize, usize)>,
    changes: usize,
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    text_input(&s.edit, TextStyle::new(16.0, 400.0)).key("editor").w(300.0).min_h(80.0)
        .on_edit(|s: &mut State| &mut s.edit)
        .on_input_selection_change(|s: &mut State, _| s.selections.push((s.edit.anchor, s.edit.focus)))
        .on_input_change(|s: &mut State, _| s.changes += 1)
}

fn runtime() -> Runtime<State> {
    let mut edit = TextEdit::new("abcdef").rich();
    edit.focused = true;
    let mut rt = Runtime::new(State { edit, ..Default::default() }, view, (400.0, 200.0), Env::new(true, 2.0));
    rt.frame(0.0);
    rt
}

#[test]
fn selection_callback_reports_each_keyboard_event_without_text_change() {
    let mut rt = runtime();
    rt.key(KeyEvent { key: Key::Left, mods: Mods::default() });
    rt.key(KeyEvent { key: Key::Left, mods: Mods::default() });
    rt.frame(0.0);
    assert_eq!(rt.state.selections, vec![(5, 5), (4, 4)]);
    assert_eq!(rt.state.changes, 0);
}

#[test]
fn selection_callback_reports_pointer_programmatic_and_history_changes() {
    let mut rt = runtime();
    rt.update(|s, _| s.edit.set_selection(1, 3));
    rt.frame(0.0);
    assert_eq!(rt.state.selections.last(), Some(&(1, 3)));
    let before = rt.state.selections.len();
    let (x, y) = rt.find("editor").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
    assert!(rt.state.selections.len() > before);
    rt.update(|s, _| { s.edit.set_caret(6); s.edit.insert("X"); });
    rt.frame(0.0);
    rt.key(KeyEvent { key: Key::Command('z'), mods: Mods { cmd: true, ..Default::default() } });
    rt.frame(0.0);
    assert_eq!(rt.state.selections.last(), Some(&(6, 6)));
    rt.key(KeyEvent { key: Key::Command('z'), mods: Mods { cmd: true, shift: true, ..Default::default() } });
    rt.frame(0.0);
    assert_eq!(rt.state.selections.last(), Some(&(7, 7)));
}

#[test]
fn selection_callback_reports_ime_cursor_and_commit_without_repeating_idle() {
    let mut rt = runtime();
    rt.ime(Ime::Preedit("日本".into(), Some((0, 3))));
    rt.frame(0.0);
    let first = rt.state.selections.len();
    assert_eq!(first, 1);
    rt.ime(Ime::Preedit("日本".into(), Some((3, 6))));
    rt.frame(0.0);
    assert_eq!(rt.state.selections.len(), first + 1);
    rt.ime(Ime::Commit("日本".into()));
    rt.frame(0.0);
    assert_eq!(rt.state.selections.last(), Some(&(12, 12)));
    let last = rt.state.selections.len();
    rt.frame(0.0);
    rt.frame(0.0);
    assert_eq!(rt.state.selections.len(), last);
}

#[test]
fn selection_callback_reports_accessibility_selection() {
    use crate::accesskit::{Action, ActionData, TextPosition, TextSelection, Role};
    let mut rt = runtime();
    let tree = rt.accessibility_tree();
    let run = tree.nodes.iter().find(|(_, n)| n.role() == Role::TextRun).unwrap().0;
    assert!(rt.accessibility_action("editor", Action::SetTextSelection,
        Some(ActionData::SetTextSelection(TextSelection {
            anchor: TextPosition { node: run, character_index: 1 },
            focus: TextPosition { node: run, character_index: 4 },
        }))));
    rt.frame(0.0);
    assert_eq!(rt.state.selections.last(), Some(&(1, 4)));
}

#[test]
fn transient_source_in_a_runtime_keeps_redo_and_returns_the_selection_map() {
    use crate::{MapBias, TransactionOptions};
    let mut rt = runtime();
    rt.update(|s, _| s.edit.insert("X"));
    rt.frame(0.0);
    rt.key(KeyEvent { key: Key::Command('z'), mods: Mods { cmd: true, ..Default::default() } });
    rt.frame(0.0);
    rt.update(|s, _| {
        let map = s.edit.transact(TransactionOptions::Transient, |e| {
            e.replace_range(0..0, "**");
            e.replace_range(e.text.len()..e.text.len(), "**");
        }).unwrap();
        assert_eq!(map.map(3, MapBias::After), 5);
    });
    rt.frame(0.0);
    assert_eq!(rt.state.edit.text, "**abcdef**");
    rt.key(KeyEvent { key: Key::Command('z'), mods: Mods { cmd: true, shift: true, ..Default::default() } });
    rt.frame(0.0);
    assert_eq!(rt.state.edit.text, "abcdefX");
}
