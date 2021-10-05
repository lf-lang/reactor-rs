use std::cmp::Reverse;


use smallvec::SmallVec;



use super::Event;


/// A queue of pending [Event]s. Events are ordered by tag,
/// so this is not a FIFO queue.
#[derive(Default)]
pub(in super) struct EventQueue<'x> {

    /// This list is sorted by the tag of each TagExecutionPlan.
    /// The earliest tag is at the end, to minimize insertions
    /// at the beginning (which would shift all remaining events
    /// right).
    ///
    /// But insertion is at worse O(n)... And it's easy to build
    /// a pathological program where this worse case is always hit.
    /// Theoretically using a tree/ heap would be useful here.
    ///
    /// Note that the size of 2 for this SmallVec was found
    /// to be the best for the Savina pong benchmark.
    value_list: SmallVec<[Event<'x>; 2]>,
}


impl<'x> EventQueue<'x> {
    /// Removes and returns the earliest tag
    pub fn take_earliest(&mut self) -> Option<Event<'x>> {
        self.value_list.pop()
    }

    /// Push an event into the heap.
    pub(in super) fn push(&mut self, evt: Event<'x>) {
        match self.value_list.binary_search_by_key(&Reverse(evt.tag), |e| Reverse(e.tag)) {
            Ok(idx) => self.value_list[idx].absorb(evt),
            Err(idx) => self.value_list.insert(idx, evt),
        }
    }
}
