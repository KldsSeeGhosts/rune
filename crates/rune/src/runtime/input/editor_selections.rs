//! Selection notifications read committed input snapshots, never stale edit accessors.

use super::*;

#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    selection: crate::EditorSelection,
    history: u64,
    composition: Option<(usize, Option<(usize, usize)>)>,
}

impl Stamp {
    fn of(edit: &crate::TextEdit) -> Self {
        Self { selection: edit.selection(), history: edit.history_movement(), composition: edit.preedit().map(|(s, c)| (s.len(), c)) }
    }
}

#[derive(Clone)]
struct Field {
    path: String,
    stamp: Stamp,
    callback: crate::el::PressFn,
}

#[derive(Default)]
pub(super) struct EditorSelections {
    observed: HashMap<String, Stamp>,
}

impl<S: 'static> Runtime<S> {
    /// Pick one changed, currently mounted input. Rebuild after its callback before choosing
    /// another, since a callback can remove itself or a different observed input.
    pub(super) fn next_editor_selection(&mut self, notified: &HashSet<String>) -> Option<(String, crate::el::PressFn)> {
        let mut fields = Vec::new();
        let mut active = HashSet::default();
        for (path, input) in self.nodes.iter().filter_map(|n| Some((&n.path, n.input.as_deref()?)))
            .chain(self.virtual_inputs.iter().map(|(p, i)| (p, i.as_ref())))
        {
            let (Some(_), Some(callback)) = (&input.access, &input.selection_change) else { continue };
            if input.edit.is_secure() || !active.insert(path.clone()) { continue; }
            fields.push(Field { path: path.clone(), stamp: Stamp::of(&input.edit), callback: callback.clone() });
        }
        self.editor_selections.observed.retain(|path, _| active.contains(path));
        for field in fields {
            let Some(previous) = self.editor_selections.observed.get(&field.path) else {
                self.editor_selections.observed.insert(field.path, field.stamp);
                continue;
            };
            if notified.contains(&field.path) {
                // Callback-owned rewrites settle in the same notification transaction.
                self.editor_selections.observed.insert(field.path, field.stamp);
                continue;
            }
            if previous == &field.stamp { continue; }
            self.editor_selections.observed.insert(field.path.clone(), field.stamp);
            return Some((field.path, field.callback));
        }
        None
    }
}
