
use std::collections::VecDeque;



use super::Event;

/// A queue of pending [Event]s. Events are ordered by tag,
/// so this is not a FIFO queue.
#[derive(Default)]
pub(in super) struct EventQueue<'x> {
    /// This list is sorted by the tag of each event (in ascending order).
    ///
    /// But insertion is at worse O(n)... And it's easy to build
    /// a pathological program where this worse case is always hit.
    /// Theoretically using a tree/ heap would be useful here.
    ///        .
    ///       ..
    ///      ...
    ///     ....
    value_list: VecDeque<Event<'x>>,
}


impl<'x> EventQueue<'x> {
    /// Removes and returns the earliest tag
    pub fn take_earliest(&mut self) -> Option<Event<'x>> {
        self.value_list.pop_front()
    }

    // todo perf: we could make a more optimal function to push a
    //  lot of events at once. Consider the following algorithm:
    //  - start with a sorted `self.value_list` and a (non-sorted) `new_evts: Vec<Event>`
    //  - sort the new events in place (in a Cow maybe=. They'll
    //  probably come in already sorted but we can't assume this.
    //  Use an algorithm that best-cases for sorted data. (eg https://crates.io/crates/dmsort)
    //  - take the earliest new event and binary search to insert it.
    //  - then do the same thing but only on the remaining (to the right)
    //  portion of `self.value_list`. Basically the routine of an insertion
    //  sort.

    /// Push an event into the heap.
    pub(in super) fn push(&mut self, evt: Event<'x>) {
        match self.value_list.binary_search_by_key(&evt.tag, |e| e.tag) {
            Ok(idx) => self.value_list[idx].absorb(evt),
            Err(idx) => self.value_list.insert(idx, evt),
        }
    }
}
