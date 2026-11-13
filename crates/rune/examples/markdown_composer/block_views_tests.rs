use super::*;
use rune::{BlockId, Env, Key, KeyEvent, Mods, Runtime};

fn runtime() -> Runtime<State> {
    let mut rt = Runtime::new(State::default(), view, (680.0, 460.0), Env::new(true, 2.0));
    rt.frame(0.0);
    rt
}

fn code(s: &State) -> rune::LogicalBlock {
    s.edit.logical_blocks().find(|b| b.kind == BlockKind::Code).unwrap()
}

fn language(s: &State) -> String { code(s).attributes.code().unwrap().info().to_owned() }

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
}

fn history(rt: &mut Runtime<State>, redo: bool) {
    rt.key(KeyEvent { key: Key::Command('z'), mods: Mods { cmd: true, shift: redo, ..Default::default() } });
    rt.frame(0.0);
}

fn header(id: BlockId) -> String { format!("language-{}", id.get()) }

#[test]
fn the_code_header_cycles_three_languages_and_preserves_exact_fence_source() {
    let mut rt = runtime();
    let id = code(&rt.state).id;
    let elements = rt.elements();
    let field = elements.iter().find(|e| e.key.as_deref() == Some("composer")).unwrap();
    let header_key = header(id);
    let control = elements.iter().find(|e| e.key.as_deref() == Some(header_key.as_str())).unwrap();
    assert!((control.rect[2] - (field.rect[2] - 32.0)).abs() < 0.1);
    rt.update(move |s, _| {
        let b = code(s);
        let attrs = b.attributes.with_code(CodeAttributes::new("~~~~", "rust  title=demo")
            .with_close(None));
        s.edit.update_logical_block(id, BlockKind::Code, attrs);
        s.edit.clear_history();
    });
    rt.frame(0.0);
    let before = rt.state.edit.selection();
    for language in ["typescript", "python", "rust"] {
        press(&mut rt, &header(id));
        let b = code(&rt.state);
        let c = b.attributes.code().unwrap();
        assert_eq!(b.id, id);
        assert_eq!(c.fence(), "~~~~");
        assert_eq!(c.info(), format!("{language}  title=demo"));
        assert_eq!(c.close(), None);
        assert_eq!(rt.state.edit.selection(), before);
    }
    history(&mut rt, false);
    assert_eq!(language(&rt.state), "python  title=demo");
    history(&mut rt, true);
    assert_eq!(language(&rt.state), "rust  title=demo");
}

#[test]
fn the_task_uses_a_leading_control_with_history_and_no_caret_movement() {
    let mut rt = runtime();
    let task = rt.state.edit.logical_blocks().find(|b| matches!(b.kind, BlockKind::Task { .. })).unwrap();
    let before = rt.state.edit.selection();
    press(&mut rt, &format!("task-{}", task.id.get()));
    assert_eq!(rt.state.edit.selection(), before);
    assert!(matches!(rt.state.edit.logical_blocks().find(|b| b.id == task.id).unwrap().kind,
        BlockKind::Task { checked: true, .. }));
    history(&mut rt, false);
    assert!(matches!(rt.state.edit.logical_blocks().find(|b| b.id == task.id).unwrap().kind,
        BlockKind::Task { checked: false, .. }));
    history(&mut rt, true);
    assert!(matches!(rt.state.edit.logical_blocks().find(|b| b.id == task.id).unwrap().kind,
        BlockKind::Task { checked: true, .. }));
}

#[test]
fn source_reveal_and_header_edits_keep_undo_redo_and_the_same_attachment() {
    let mut rt = runtime();
    let id = code(&rt.state).id;
    rt.update(|s, _| s.edit.set_caret(s.edit.text.find("bold").unwrap() + 2));
    rt.frame(0.0);
    assert!(rt.state.edit.text.contains("**bold**"));
    press(&mut rt, &header(id));
    assert_eq!(language(&rt.state), "typescript title=demo");
    history(&mut rt, false);
    assert_eq!(language(&rt.state), "rust title=demo");
    assert!(rt.find(&header(id)).is_some());
    history(&mut rt, true);
    assert_eq!(language(&rt.state), "typescript title=demo");
    assert!(rt.find(&header(id)).is_some());
    rt.update(|s, _| s.edit.set_caret(0));
    rt.frame(0.0);
    assert!(!rt.state.edit.text.contains("**"));
}

#[test]
fn changing_language_does_not_cancel_a_code_blocks_composition() {
    let mut rt = runtime();
    let id = code(&rt.state).id;
    rt.ime(rune::Ime::Preedit("日本".into(), Some((0, 3))));
    rt.frame(0.0);
    let before = rt.state.edit.selection();
    press(&mut rt, &header(id));
    assert_eq!(rt.state.edit.selection(), before);
    assert_eq!(rt.state.edit.preedit(), Some(("日本", Some((0, 3)))));
    assert_eq!(language(&rt.state), "typescript title=demo");
    rt.ime(rune::Ime::Commit("日本".into()));
    rt.frame(0.0);
    assert!(rt.state.edit.text.ends_with("日本"));
    assert_eq!(code(&rt.state).id, id);
}
