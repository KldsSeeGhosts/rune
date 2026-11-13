//! Selection notifications share one observer registry, outside the edit accessor.

use super::*;

#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    anchor: usize,
    focus: usize,
    composition: Option<(usize, Option<(usize, usize)>)>,
}

impl Stamp {
    fn of(edit: &crate::TextEdit) -> Self {
        Self { anchor: edit.anchor, focus: edit.focus, composition: edit.preedit().map(|(s, c)| (s.len(), c)) }
    }
}

#[derive(Clone)]
struct Field {
    path: String,
    access: crate::el::EditFn,
    callback: crate::el::PressFn,
}

#[derive(Default)]
pub(super) struct EditorSelections {
    fields: Vec<Field>,
    observed: HashMap<String, Stamp>,
}

impl<S: 'static> Runtime<S> {
    /// Refresh only when a view commits. Fields without callbacks add no per-input traversal.
    pub(super) fn register_editor_selections(&mut self) {
        let mut fields = Vec::new();
        let mut active = HashSet::default();
        for (path, input) in self.nodes.iter().filter_map(|n| Some((&n.path, n.input.as_deref()?)))
            .chain(self.virtual_inputs.iter().map(|(p, i)| (p, i.as_ref())))
        {
            let (Some(access), Some(callback)) = (&input.access, &input.selection_change) else { continue };
            if input.edit.is_secure() || !active.insert(path.clone()) { continue; }
            self.editor_selections.observed.entry(path.clone()).or_insert_with(|| Stamp::of(&input.edit));
            fields.push(Field { path: path.clone(), access: access.clone(), callback: callback.clone() });
        }
        self.editor_selections.observed.retain(|path, _| active.contains(path));
        self.editor_selections.fields = fields;
    }

    /// After each action, update or poll. A callback sees the final edit, not intermediary
    /// accessor calls. Rewrites made by that callback settle in the same notification.
    pub(super) fn notify_editor_selections(&mut self, tx: &mut Tx) -> bool {
        if self.editor_selections.fields.is_empty() { return false; }
        let fields = self.editor_selections.fields.clone();
        let mut changed = false;
        for field in fields {
            let edit = (field.access)(states!(self));
            if edit.is_secure() { continue; }
            let stamp = Stamp::of(edit);
            if self.editor_selections.observed.get(&field.path) == Some(&stamp) { continue; }
            self.editor_selections.observed.insert(field.path.clone(), stamp);
            (field.callback)(states!(self), tx);
            let stamp = Stamp::of((field.access)(states!(self)));
            self.editor_selections.observed.insert(field.path, stamp);
            changed = true;
        }
        changed
    }
}
