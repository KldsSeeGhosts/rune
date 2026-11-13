//! Transaction capture modes. Entering secure mode invalidates the whole capture.

use super::{EditMap, Range};
use crate::machine::Effects;

#[derive(Default)]
pub(in crate::edit) enum State {
    #[default]
    Idle,
    Active { map: EditMap, explicit_selection: bool },
    Secured,
}

impl Clone for State {
    fn clone(&self) -> Self { Self::Idle }
}
impl PartialEq for State {
    fn eq(&self, _: &Self) -> bool { true }
}

pub(super) enum Event {
    Begin,
    Replace { range: Range<usize>, inserted: usize },
    Selection,
    Secure,
    Finish,
    Abort,
}

pub(super) enum Effect {
    Finished { map: EditMap, explicit_selection: bool },
    Secured,
}

pub(super) fn step(state: &mut State, event: Event) -> Effects<Effect> {
    match event {
        Event::Begin => {
            *state = State::Active { map: EditMap::default(), explicit_selection: false };
        }
        Event::Replace { range, inserted } => {
            if let State::Active { map, .. } = state { map.push(range, inserted); }
        }
        Event::Selection => {
            if let State::Active { explicit_selection, .. } = state { *explicit_selection = true; }
        }
        Event::Secure => {
            if matches!(state, State::Active { .. }) { *state = State::Secured; }
        }
        Event::Abort => *state = State::Idle,
        Event::Finish => return match std::mem::take(state) {
            State::Idle => Effects::none(),
            State::Active { map, explicit_selection } => Effects::one(Effect::Finished { map, explicit_selection }),
            State::Secured => Effects::one(Effect::Secured),
        },
    }
    Effects::none()
}
