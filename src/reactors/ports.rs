use crate::reactors::id::{AssemblyId, GlobalId, Identified};
use crate::reactors::util::Named;

pub enum PortKind { Input, Output }

pub struct PortId<T> {
    kind: PortKind,
    global_id: GlobalId,
}

impl<T> PortId<T> {
    pub(in super) fn new(kind: PortKind, global_id: GlobalId) -> Self {
        PortId::<T> { kind, global_id }
    }
}

impl<T> Identified for PortId<T> {
    fn global_id(&self) -> &GlobalId {
        &self.global_id
    }
}
