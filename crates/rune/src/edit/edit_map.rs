//! Byte-position maps produced by document transactions.

use super::*;

/// Which side of inserted or replaced text an endpoint follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapBias { Before, After }

#[derive(Clone, Debug, PartialEq, Eq)]
struct Step {
    range: Range<usize>,
    inserted: usize,
}

/// An ordered sequence of byte replacements. Each step uses its preceding document's
/// coordinates. Positions outside the source document are the caller's responsibility.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditMap { steps: Vec<Step> }

impl EditMap {
    pub fn is_empty(&self) -> bool { self.steps.is_empty() }

    pub fn map(&self, mut position: usize, bias: MapBias) -> usize {
        for step in &self.steps {
            if position < step.range.start { continue; }
            position = if position > step.range.end {
                position - step.range.len() + step.inserted
            } else {
                step.range.start + if bias == MapBias::After { step.inserted } else { 0 }
            };
        }
        position
    }

    /// Maps a range with independently chosen boundary behavior.
    pub fn map_range(&self, range: Range<usize>, start: MapBias, end: MapBias) -> Range<usize> {
        let a = self.map(range.start, start);
        let b = self.map(range.end, end);
        a..b.max(a)
    }

    /// The reverse coordinate map. Deleted source positions cannot be reconstructed uniquely;
    /// the bias chooses the appropriate edge.
    pub fn inverse(&self) -> Self {
        Self { steps: self.steps.iter().rev().map(|s| Step {
            range: s.range.start..s.range.start + s.inserted, inserted: s.range.len(),
        }).collect() }
    }

    /// Apply this map, then `next`, whose source coordinates are this map's destination.
    pub fn then(&self, next: &Self) -> Self {
        let mut result = self.clone();
        result.steps.extend(next.steps.iter().cloned());
        result
    }

    pub(super) fn push(&mut self, range: Range<usize>, inserted: usize) {
        self.steps.push(Step { range, inserted });
    }
}
