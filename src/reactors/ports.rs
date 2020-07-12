use crate::reactors::id::{AssemblyId, GlobalId};
use crate::reactors::util::Named;

pub enum PortKind { Input, Output }

pub struct PortId<T> {
    kind: PortKind,
    id: GlobalId,
}

impl<T> PortId<T> {
    pub(in super) fn new(kind: PortKind, id: GlobalId) -> Self {
        PortId::<T> { kind, id }
    }
}

impl Named for PortId<T> {
    fn name(&self) -> &'static str {
        self.id.name()
    }
}
