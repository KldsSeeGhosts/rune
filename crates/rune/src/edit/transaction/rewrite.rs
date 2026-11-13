//! Detached source state and the effects that restore canonical history coordinates.

use super::{EditMap, MapBias, Snapshot, TextEdit};
use crate::machine::Effects;

#[derive(Clone, Default, Debug, PartialEq)]
pub(in crate::edit) enum State {
    #[default]
    Canonical,
    Detached { baseline: Snapshot, map: EditMap },
}

pub(super) enum Event {
    Rewritten { baseline: Snapshot, map: EditMap },
    Consume { anchor: usize, focus: usize },
    Discard,
    Restore(State),
}

pub(super) enum Effect { Baseline(Snapshot) }

pub(super) fn step(state: &mut State, event: Event) -> Effects<Effect> {
    match event {
        Event::Rewritten { baseline, map } => {
            *state = match std::mem::take(state) {
                State::Canonical => State::Detached { baseline, map },
                State::Detached { baseline, map: previous } =>
                    State::Detached { baseline, map: previous.then(&map) },
            };
        }
        Event::Consume { anchor, focus } => {
            if let State::Detached { mut baseline, map } = std::mem::take(state) {
                let inverse = map.inverse();
                baseline.anchor = TextEdit::clamp_char_boundary(&baseline.text, inverse.map(anchor, MapBias::After));
                baseline.focus = TextEdit::clamp_char_boundary(&baseline.text, inverse.map(focus, MapBias::After));
                return Effects::one(Effect::Baseline(baseline));
            }
        }
        Event::Discard => *state = State::Canonical,
        Event::Restore(previous) => *state = previous,
    }
    Effects::none()
}
