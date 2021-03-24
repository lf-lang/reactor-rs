use std::sync::Arc;

use crate::runtime::*;
use crate::runtime::Output;

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

pub fn set_fake_downstream<T, K>(container: ReactorId, ids: Vec<u32>, port: &mut Port<T, K>) {
    port.set_downstream(make_deps(container, ids))
}

pub fn assert_deps_eq<T>(container: ReactorId, local_ids: Vec<u32>, port: &Port<T, Output>) {
    let expected =
        local_ids.into_iter()
            .map(|loc| container.make_reaction_id(loc))
            .collect::<Vec<_>>();

    let actual =
        port.get_downstream_deps().iter()
            .map(|r| r.id())
            .collect::<Vec<_>>();


    assert_eq!(expected, actual);
}
