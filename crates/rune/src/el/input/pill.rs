//! Optional pill activation, separate from ordinary field editing.
use super::*;
use std::ops::Range;

pub(crate) type PillPressFn = Rc<dyn Fn(crate::states::States<'_>, &str, Range<usize>, &mut Tx)>;

impl El {
    /// Additional activation for a pill in a bound text input. Receives its id and UTF-8
    /// range in the model, after ordinary caret/selection handling. A primary press and
    /// release must hit the same pill without a drag. A double click activates only on its
    /// first click; word selection is unchanged.
    ///
    /// Unmodified Enter or Space also activates an exactly selected pill, after the input
    /// and app key handlers get first refusal. Key repeats do not activate again. No handler
    /// means no change to pointer or keyboard editing. Secure inputs never activate pills.
    /// Use [`Cx::input_pill_range_rect`] to anchor to this occurrence, even with repeated ids.
    pub fn on_input_pill_press<S: 'static>(mut self, f: impl Fn(&mut S, &str, Range<usize>, &mut Tx) + 'static) -> Self {
        if let Some(input) = self.cold.get_mut_if_some().and_then(|c| c.input.as_deref_mut()) {
            input.pill_press = Some(Rc::new(move |s, id, range, tx| f(downcast::<S>(s), id, range, tx)));
        }
        self
    }
}
