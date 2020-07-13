use std::marker::PhantomData;

use crate::reactors::id::{GlobalId, Identified};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PortKind { Input, Output }

pub struct PortId<T> {
    kind: PortKind,
    global_id: GlobalId,
    _phantom_t: PhantomData<T>,
}

impl<T> PortId<T> {
    pub(in super) fn new(kind: PortKind, global_id: GlobalId) -> Self {
        PortId::<T> { kind, global_id, _phantom_t: PhantomData }
    }

    fn kind(&self) -> PortKind {
        self.kind
    }
}

impl<T> Identified for PortId<T> {
    fn global_id(&self) -> &GlobalId {
        &self.global_id
    }
}
