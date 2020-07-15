use std::borrow::Borrow;
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;

use crate::reactors::flowgraph::GraphId;
use crate::reactors::util::Named;
use std::ops::Deref;

/// Identifies an assembly uniquely in the tree
/// This is just a path built from the root down.
#[derive(Eq, PartialEq, Clone, Hash)]
pub enum AssemblyId {
    Root,
    Nested {
        // the id of the parent
        parent: Rc<AssemblyId>,
        // this is just for debugging
        user_name: &'static str,
    },
}

impl Display for AssemblyId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Root => write!(f, ""),
            AssemblyId::Nested { user_name, parent, .. } => {
                Debug::fmt(parent, f)?;
                write!(f, "/@{}", user_name)
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

// TODO make that more lightweight (should be Copy)
#[derive(Eq, PartialEq, Clone, Hash)]
pub struct GlobalId {
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

impl Display for GlobalId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

pub trait Identified {
    fn global_id(&self) -> &GlobalId;

    fn is_in_direct_subreactor_of(&self, reactor_id: &impl Deref<Target=AssemblyId>) -> bool {
        let my_assembly: &AssemblyId = self.global_id().assembly_id.borrow();

        my_assembly.parent().map_or(false, |it| it == reactor_id.deref())
    }

    fn is_in_reactor(&self, reactor_id: &impl Deref<Target=AssemblyId>) -> bool {
        let my_assembly: &AssemblyId = self.global_id().assembly_id.borrow();

        my_assembly == reactor_id.deref()
    }
}

impl<T> Named for T where T: Identified {
    fn name(&self) -> &'static str {
        self.global_id().name
    }
}

