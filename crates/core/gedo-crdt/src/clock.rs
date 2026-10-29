//! Clocks consistent with the commit history (research R3).

use gedo_core::event::{Clock, Event, EventId};

use crate::id::event_id;

/// The generation of a commit from its parents' generations: 1 for a root, one more than the
/// highest parent otherwise.
pub fn generation(parent_generations: &[u64]) -> u64 {
    parent_generations
        .iter()
        .max()
        .map_or(1, |highest| highest.saturating_add(1))
}

/// The clock of a live event recorded on top of a commit of `base_generation` (none: an empty
/// history), at `sequence` in the write-ahead log.
pub fn live_clock(base_generation: Option<u64>, sequence: u32) -> Clock {
    Clock {
        generation: generation(base_generation.as_slice()),
        sequence,
    }
}

/// Sorts events in the total order `(clock, id)`, the same on every machine for any input order.
pub fn sort_events(events: &mut [Event]) {
    events.sort_by_cached_key(order_key);
}

/// The key of the total order.
pub fn order_key(event: &Event) -> (Clock, EventId) {
    (event.clock, event_id(event))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generations_follow_the_highest_parent() {
        assert_eq!(generation(&[]), 1);
        assert_eq!(generation(&[1]), 2);
        assert_eq!(generation(&[3, 7, 2]), 8);
    }

    #[test]
    fn live_events_are_one_generation_above_their_base() {
        assert_eq!(
            live_clock(None, 4),
            Clock {
                generation: 1,
                sequence: 4
            }
        );
        assert_eq!(
            live_clock(Some(5), 0),
            Clock {
                generation: 6,
                sequence: 0
            }
        );
    }
}
