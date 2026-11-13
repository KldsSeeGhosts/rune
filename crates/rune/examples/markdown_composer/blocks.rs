//! Application-owned code headers and task controls, using only public editor APIs.
use super::*;
use rune::{BlockId, InputBlockView};

pub(super) fn attach(mut field: El, edit: &TextEdit) -> El {
    let theme = tok::theme(true);
    for block in edit.logical_blocks() {
        let id = block.id;
        match block.kind {
            BlockKind::Code => {
                let language = block.attributes.code().map_or("", CodeAttributes::info)
                    .split_whitespace().next().unwrap_or("plain text");
                let control = text(language).font(type_::LABEL).color(theme.text_secondary)
                    .key(format!("language-{}", id.get())).role("button")
                    .label(format!("Code language: {language}. Press to cycle.")).w_full().h(24.0)
                    .on_press(move |s: &mut State| cycle(&mut s.edit, id));
                field = field.input_block_view(InputBlockView::header(id, control));
            }
            BlockKind::Task { checked, .. } => {
                let control = text(if checked { "[x]" } else { "[ ]" }).font(type_::LABEL).color(theme.text)
                    .key(format!("task-{}", id.get())).role("checkbox").checked(checked)
                    .label("Task completed").size(24.0, 24.0)
                    .on_press(move |s: &mut State| toggle(&mut s.edit, id));
                field = field.input_block_view(InputBlockView::leading(id, control));
            }
            _ => {}
        }
    }
    field
}

fn cycle(edit: &mut TextEdit, id: BlockId) {
    let Some(block) = edit.logical_blocks().find(|b| b.id == id && b.kind == BlockKind::Code) else { return };
    let default = CodeAttributes::default();
    let code = block.attributes.code().unwrap_or(&default);
    let info = code.info();
    let end = info.find(char::is_whitespace).unwrap_or(info.len());
    let language = match &info[..end] { "rust" => "typescript", "typescript" => "python", _ => "rust" };
    let info = format!("{language}{}", &info[end..]);
    let code = CodeAttributes::new(code.fence(), info).with_close(code.close().map(str::to_owned));
    edit.update_logical_block(id, BlockKind::Code, block.attributes.with_code(code));
}

fn toggle(edit: &mut TextEdit, id: BlockId) {
    let Some(block) = edit.logical_blocks().find(|b| b.id == id) else { return };
    if let BlockKind::Task { level, checked } = block.kind {
        edit.update_logical_block(id, BlockKind::Task { level, checked: !checked }, block.attributes);
    }
}
