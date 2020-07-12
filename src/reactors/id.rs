use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;
use std::borrow::Borrow;
use crate::reactors::util::Named;

/// Identifies an assembly uniquely in the tree
/// This is just a path built from the root down.
#[derive(Eq, PartialEq, Clone)]
pub(super) enum AssemblyId {
    Root,
    Nested {
        // This is the node id used in the parent
        ext_id: NodeId,
        // the id of the parent
        parent: Rc<AssemblyId>,

        // this is just for debugging
        typename: &'static str,
    },
}

impl Display for AssemblyId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Root => write!(f, ""),
            AssemblyId::Nested { typename, ext_id, parent } => {
                Debug::fmt(parent, f)?;
                write!(f, "/{}[{}]", typename, ext_id.index())
            }
        }
    }
}

impl Debug for AssemblyId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl AssemblyId {
    fn parent(&self) -> Option<&AssemblyId> {
        match self {
            Self::Root => None,
            Self::Nested { parent, .. } => Some(Rc::borrow(parent)),
        }
    }
}

#[derive(Eq, PartialEq, Clone)]
pub(super) struct GlobalId {
    assembly_id: Rc<AssemblyId>,
    name: &'static str,
}

impl GlobalId {
    pub(super) fn new(assembly_id: Rc<AssemblyId>, name: &'static str) -> GlobalId {
        GlobalId { assembly_id, name }
    }
}

impl Named for GlobalId {
    fn name(&self) -> &'static str {
        self.name
    }
}

impl Debug for GlobalId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.assembly_id, f)?;
        write!(f, "/@{}", self.name)
    }
}


