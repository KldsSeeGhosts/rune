use super::*;
use crate::{Key, Mods, TextEdit, TextStyle, text_input};

// Share Runtime<State> with block-view tests. Another full runtime monomorphization makes
// the native library-test executable exceed the macOS linker's practical size limits.
pub(super) struct State {
    pub(super) edit: TextEdit,
    pub(super) selections: Vec<(usize, usize)>,
    pub(super) changes: usize,
    pub(super) code: crate::BlockId,
    pub(super) task: crate::BlockId,
    pub(super) header_height: f32,
    pub(super) views: bool,
    pub(super) opt_out: bool,
    pub(super) presses: usize,
    pub(super) secure: bool,
}

impl Default for State {
    fn default() -> Self {
        let edit = TextEdit::default();
        let id = edit.logical_blocks().next().unwrap().id;
        Self { edit, selections: Vec::new(), changes: 0, code: id, task: id,
            header_height: 32.0, views: false, opt_out: false, presses: 0, secure: false }
    }
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

#[test]
fn mark_only_history_movement_notifies_even_at_identical_selection_offsets() {
    let mut rt = runtime();
    rt.update(|s, _| {
        s.edit.set_selection(1, 4);
        s.edit.set_mark(crate::Mark::Bold);
    });
    rt.frame(0.0);
    let before = rt.state.selections.len();
    rt.key(KeyEvent { key: Key::Command('z'), mods: Mods { cmd: true, ..Default::default() } });
    rt.frame(0.0);
    assert_eq!(rt.state.selections.len(), before + 1);
    assert_eq!(rt.state.selections.last(), Some(&(1, 4)));
    rt.key(KeyEvent { key: Key::Command('z'), mods: Mods { cmd: true, shift: true, ..Default::default() } });
    rt.frame(0.0);
    assert_eq!(rt.state.selections.len(), before + 2);
    assert_eq!(rt.state.changes, 0);
}

#[derive(Default)]
struct Dynamic {
    edit: Option<TextEdit>,
    notifications: usize,
}

fn dynamic_view(s: &Dynamic, _: &mut Cx<Dynamic>) -> El {
    match &s.edit {
        Some(edit) => text_input(edit, TextStyle::new(16.0, 400.0)).key("dynamic")
            .on_edit(|s: &mut Dynamic| s.edit.as_mut().unwrap())
            .on_input_selection_change(|s: &mut Dynamic, _| {
                s.notifications += 1;
                s.edit = None;
            }),
        None => crate::col(),
    }
}

#[test]
fn unmounting_an_observed_field_does_not_call_its_stale_accessor() {
    let mut rt = Runtime::new(Dynamic { edit: Some(TextEdit::new("abc")), ..Default::default() },
        dynamic_view, (300.0, 100.0), Env::new(true, 2.0));
    rt.frame(0.0);
    rt.update(|s, _| s.edit = None);
    rt.frame(0.0);
    assert_eq!(rt.state.notifications, 0);
}

#[test]
fn a_selection_callback_can_remove_its_own_editor() {
    let mut rt = Runtime::new(Dynamic { edit: Some(TextEdit::new("abc")), ..Default::default() },
        dynamic_view, (300.0, 100.0), Env::new(true, 2.0));
    rt.frame(0.0);
    rt.update(|s, _| s.edit.as_mut().unwrap().set_caret(0));
    rt.frame(0.0);
    assert_eq!(rt.state.notifications, 1);
    assert!(rt.state.edit.is_none());
}
