use super::*;
use rune::{Env, Key, KeyEvent, Mods, Runtime};

#[test]
fn the_sample_keeps_blocks_and_source_reveal_does_not_destroy_redo() {
    let mut rt = Runtime::new(State::default(), view, (680.0, 460.0), Env::new(true, 2.0));
    rt.frame(0.0);
    let kinds = rt.state.edit.logical_blocks().map(|b| b.kind).collect::<Vec<_>>();
    assert_eq!(kinds[0], BlockKind::Heading { level: 1 });
    assert_eq!(kinds[1], BlockKind::Task { level: 0, checked: false });
    assert_eq!(kinds[2], BlockKind::Rule);
    assert_eq!(kinds.last(), Some(&BlockKind::Code));
    rt.update(|s, _| s.edit.insert("X"));
    rt.frame(0.0);
    let command = |shift| KeyEvent { key: Key::Command('z'), mods: Mods { cmd: true, shift, ..Default::default() } };
    rt.key(command(false));
    rt.frame(0.0);
    rt.update(|s, _| {
        let at = s.edit.text.find("bold").unwrap();
        s.edit.set_caret(at + 2);
    });
    rt.frame(0.0);
    assert!(rt.state.edit.text.contains("**bold**"));
    assert_eq!(rt.state.edit.app_marks().count(), 1);
    rt.key(command(true));
    rt.frame(0.0);
    assert!(rt.state.edit.text.ends_with('X'));
    assert!(!rt.state.edit.text.contains("**bold**"));
    let code = rt.state.edit.logical_blocks().find(|b| b.kind == BlockKind::Code).unwrap();
    assert_eq!(code.attributes.code().unwrap().info(), "rust title=demo");
}

#[test]
fn leaving_source_collapses_it_and_an_edit_undo_restores_the_styled_word() {
    let mut rt = Runtime::new(State::default(), view, (680.0, 460.0), Env::new(true, 2.0));
    rt.frame(0.0);
    rt.update(|s, _| {
        let at = s.edit.text.find("bold").unwrap();
        s.edit.set_caret(at + 2);
    });
    rt.frame(0.0);
    rt.key(KeyEvent { key: Key::Text("X".into()), mods: Mods::default() });
    rt.frame(0.0);
    assert!(rt.state.edit.text.contains("**boXld**"));
    rt.update(|s, _| s.edit.set_caret(0));
    rt.frame(0.0);
    assert!(!rt.state.edit.text.contains("**"));
    rt.key(KeyEvent { key: Key::Command('z'), mods: Mods { cmd: true, ..Default::default() } });
    rt.frame(0.0);
    assert!(!rt.state.edit.text.contains("boXld"));
}
