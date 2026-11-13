//! Markdown composer APIs. Run `cargo run -p rune --example markdown_composer`.
//! The source example handles one balanced bold region, not general Markdown parsing.

use std::ops::Range;
use rune::machine::Effects;
use rune::tok::{self, type_};
use rune::{
    AppMark, AppSpec, BlockAttributes, BlockKind, CodeAttributes, Cx, El, Mark, MarkRange,
    TextEdit, TransactionOptions, Tx, col, text, text_input,
};

#[path = "markdown_composer/blocks.rs"]
mod blocks;

struct State { edit: TextEdit }

impl Default for State {
    fn default() -> Self {
        let mut edit = TextEdit::new(concat!(
            "Composer example\nTry the checkbox\n\nEdit the bold word.\n",
            "let answer = 42;\nprintln!(\"{answer}\");"
        )).rich();
        edit.set_logical_block(edit.line_range(0), BlockKind::Heading { level: 1 },
            BlockAttributes::default().with_heading_space(" ")).unwrap();
        edit.set_logical_block(edit.line_range(1), BlockKind::Task { level: 0, checked: false },
            BlockAttributes::default().with_marker_space(" ").with_content_space(" ")).unwrap();
        edit.set_logical_block(edit.line_range(2), BlockKind::Rule,
            BlockAttributes::default().with_rule_source("---")).unwrap();
        edit.set_logical_block(edit.line_range(4).start..edit.text.len(), BlockKind::Code,
            BlockAttributes::default().with_code(CodeAttributes::new("```", "rust title=demo"))).unwrap();
        let bold = edit.text.find("bold").unwrap();
        edit.set_marks(vec![MarkRange::new(bold..bold + 4, Mark::Bold)]);
        edit.clear_history();
        edit.focused = true;
        Self { edit }
    }
}

enum Source { Styled, Visible(Range<usize>) }
enum Event { Selection { anchor: usize, focus: usize, bold: Option<Range<usize>> } }
enum Rewrite { Reveal(Range<usize>), Collapse(Range<usize>) }

fn touches(anchor: usize, focus: usize, r: &Range<usize>) -> bool {
    if anchor == focus { r.start <= focus && focus <= r.end }
    else { anchor.min(focus) < r.end && anchor.max(focus) > r.start }
}

fn step(source: &mut Source, event: Event) -> Effects<Rewrite> {
    let Event::Selection { anchor, focus, bold } = event;
    match source {
        Source::Visible(r) if !touches(anchor, focus, r) => {
            let r = r.clone();
            *source = Source::Styled;
            Effects::one(Rewrite::Collapse(r))
        }
        Source::Styled => {
            if let Some(r) = bold.filter(|r| touches(anchor, focus, r)) {
                *source = Source::Visible(r.clone());
                Effects::one(Rewrite::Reveal(r))
            } else { Effects::none() }
        }
        _ => Effects::none(),
    }
}

fn normalize(s: &mut State, _: &mut Tx) {
    let e = &mut s.edit;
    if e.is_composing() { return; }
    // History owns the authoritative mode through this app mark, not separate UI state.
    let mut source = e.app_marks().find(|(_, m)| m.id() == "markdown-source")
        .map_or(Source::Styled, |(r, _)| Source::Visible(r));
    let bold = e.marks().iter().find(|m| m.mark == Mark::Bold).map(MarkRange::range);
    for effect in step(&mut source, Event::Selection { anchor: e.anchor, focus: e.focus, bold }) {
        match effect {
            Rewrite::Reveal(r) => {
                e.transact(TransactionOptions::Transient, |e| {
                    e.replace_range(r.end..r.end, "**");
                    e.replace_range(r.start..r.start, "**");
                    let range = r.start..r.end + 4;
                    e.set_marks(e.marks().iter().filter(|m| {
                        m.mark != Mark::Bold || m.end <= range.start || m.start >= range.end
                    }).cloned().collect());
                    e.set_app_mark(range, AppMark::new("markdown-source"));
                }).unwrap();
            }
            Rewrite::Collapse(r) => {
                let balanced = e.text.get(r.clone()).is_some_and(|s| s.len() >= 4 && s.starts_with("**") && s.ends_with("**"));
                e.transact(TransactionOptions::Transient, |e| {
                    e.remove_app_mark("markdown-source");
                    if balanced {
                        e.replace_range(r.end - 2..r.end, "");
                        e.replace_range(r.start..r.start + 2, "");
                        let mut marks = e.marks().to_vec();
                        if r.end - 4 > r.start { marks.push(MarkRange::new(r.start..r.end - 4, Mark::Bold)); }
                        e.set_marks(marks);
                    }
                }).unwrap();
            }
        }
    }
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let t = tok::theme(true);
    let composer = text_input(&s.edit, type_::BODY).key("composer").w_full().min_h(260.0)
        .padding(16.0).fill(t.card).color(t.text)
        .on_edit(|s: &mut State| &mut s.edit)
        .on_submit(|s: &mut State, _| s.edit.insert("\n"))
        .on_input_change(normalize)
        .on_input_selection_change(normalize);
    col().w_full().h_full().padding(24.0).gap(12.0).fill(t.bg)
        .child(text("Markdown composer").font(type_::LABEL).color(t.text))
        .child(text("Click the checkbox or rule. Enter the bold word to reveal its source. Undo and redo keep working.")
            .font(type_::BODY).color(t.text_secondary).wrap().w_full())
        .child(blocks::attach(composer, &s.edit))
        .child(text("Press a code language to cycle it. Tab visits block controls. Escape returns to the editor.")
            .font(type_::LABEL).color(t.text_secondary).wrap().w_full())
}

fn main() {
    rune::run(AppSpec::new("Markdown composer", (680.0, 460.0), State::default, view));
}

#[cfg(test)]
#[path = "markdown_composer/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "markdown_composer/block_views_tests.rs"]
mod block_views_tests;
