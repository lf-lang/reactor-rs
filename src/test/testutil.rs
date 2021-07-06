//! Test utilities.

use std::sync::Arc;

use crate::*;
use crate::Output;

/// Set a port to a value
pub fn set_port<T>(port: &mut OutputPort<T>, v: T) {
    port.set_impl(v, |_| {})
}

fn make_deps(container: ReactorId, ids: Vec<u32>) -> ToposortedReactions {
    let mut result = Vec::new();
    for id in ids.iter() {
        let r = ReactionInvoker::new_from_closure(container, *id, |_| {});
        result.push(Arc::new(r));
    }
    result
}

/// Set the given port's downstream dependencies as a set of
/// fake reactions whose ids are exactly the given `local_ids`,
/// taken to represent reactions of the given reactor.
pub fn set_fake_downstream<T, K>(container: ReactorId, ids: Vec<u32>, port: &mut Port<T, K>) {
    port.set_downstream(make_deps(container, ids))
}

/// Assert that the given port's recorded downstream dependencies
/// have exactly the ids contained in the given `local_ids`,
/// taken to represent reactions of the given reactor.
pub fn assert_deps_eq<T>(container: ReactorId, local_ids: Vec<u32>, port: &Port<T, Output>) {
    let expected =
        local_ids.into_iter()
            .map(|loc| container.make_reaction_id(loc))
            .collect::<Vec<_>>();

    let actual =
        port.get_downstream_deps().iter()
            .map(|r| r.id())
            .collect::<Vec<_>>();


    assert_eq!(expected, actual, "Reaction IDs do not match");
}
