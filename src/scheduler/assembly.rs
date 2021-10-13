/*
 * Copyright (c) 2021, TU Dresden.
 *
 * Redistribution and use in source and binary forms, with or without modification,
 * are permitted provided that the following conditions are met:
 *
 * 1. Redistributions of source code must retain the above copyright notice,
 *    this list of conditions and the following disclaimer.
 *
 * 2. Redistributions in binary form must reproduce the above copyright notice,
 *    this list of conditions and the following disclaimer in the documentation
 *    and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY
 * EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL
 * THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
 * PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF
 * THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */



use std::borrow::Cow;
use crate::*;
use crate::scheduler::depgraph::DepGraph;

use super::ReactorVec;

pub(in super) struct RootAssembler {
    /// ID of the next reactor to assign
    reactor_id: ReactorId,
    /// All registered reactors
    pub(in super) reactors: ReactorVec<'static>,
    /// Dependency graph
    pub(in super) graph: DepGraph,
    pub(in super) id_registry: IdRegistry,
}

impl Default for RootAssembler {
    fn default() -> Self {
        Self {
            reactor_id: ReactorId::new(0),
            graph: DepGraph::new(),
            id_registry: Default::default(),
            reactors: Default::default(),
        }
    }
}


/// Helper struct to assemble reactors during initialization.
/// One assembly context is used per reactor, they can't be shared.
pub struct AssemblyCtx<'x> {
    globals: &'x mut RootAssembler,
    /// Constant id of the reactor currently being built.
    reactor_id: Option<ReactorId>,
    /// Next local ID for components != reactions
    cur_local: LocalReactionId,
    reactions_done: bool,

    // debug info:

    debug: ReactorDebugInfo,
}

impl<'x> AssemblyCtx<'x> {
    /// The ID of the reactor being built.
    ///
    /// ### Panics
    /// If fix_cur_id has not been called.
    pub fn get_id(&self) -> ReactorId {
        self.reactor_id.unwrap_or_else(|| panic!("fix_cur_id has not been called"))
    }

    /// Note: this needs to be called after all children reactors
    /// have been built, as they're pushed into the global reactor
    /// vec before their parent. So the ID of the parent needs to
    /// be fixed only after all descendants have been built.
    pub fn fix_cur_id(&mut self) -> ReactorId {
        let id = self.globals.reactor_id;
        self.reactor_id = Some(id);
        self.globals.reactor_id += 1;
        self.globals.id_registry.record_reactor(id, &self.debug);
        id
    }

    pub fn new_port<T: Send>(&mut self, lf_name: &'static str) -> Port<T> {
        let id = self.next_comp_id(Some(Cow::Borrowed(lf_name)));
        self.globals.graph.record_port(id);
        Port::new(id)
    }

    fn new_port_impl<T: Send>(&mut self, lf_name: Cow<'static, str>) -> Port<T> {
        let id = self.next_comp_id(Some(lf_name));
        self.globals.graph.record_port(id);
        Port::new(id)
    }

    pub fn new_port_bank<T: Send, const N: usize>(&mut self, lf_name: &'static str) -> [Port<T>; N] {
        array![i => {
            let label = Cow::Owned(format!("{}[{}]", lf_name, i));
            self.new_port_impl::<T>(label)
        } ; N]
    }

    pub fn new_logical_action<T: Send>(&mut self,
                                       lf_name: &'static str,
                                       min_delay: Option<Duration>) -> LogicalAction<T> {
        let id = self.next_comp_id(Some(Cow::Borrowed(lf_name)));
        self.globals.graph.record_laction(id);
        LogicalAction::new(id, min_delay)
    }

    pub fn new_physical_action<T: Send>(&mut self,
                                        lf_name: &'static str,
                                        min_delay: Option<Duration>) -> PhysicalActionRef<T> {
        let id = self.next_comp_id(Some(Cow::Borrowed(lf_name)));
        self.globals.graph.record_paction(id);
        PhysicalActionRef::new(id, min_delay)
    }

    pub fn new_timer(&mut self, lf_name: &'static str, offset: Duration, period: Duration) -> Timer {
        let id = self.next_comp_id(Some(Cow::Borrowed(lf_name)));
        self.globals.graph.record_timer(id);
        Timer::new(id, offset, period)
    }

    /// Create N reactions. The first `num_non_synthetic` get
    /// priority edges, as they are taken to be those declared
    /// in LF by the user.
    /// The rest do not have priority edges, and their
    /// implementation must hence have no observable side-effect.
    pub fn new_reactions<const N: usize>(&mut self,
                                         num_non_synthetic: usize,
                                         names: [Option<&'static str>; N]) -> [GlobalReactionId; N] {
        assert!(!self.reactions_done, "May only create reactions once");
        self.reactions_done = true;

        let result = array![i => GlobalReactionId::new(self.get_id(), LocalReactionId::new(i)); N];

        let mut prev: Option<GlobalReactionId> = None;
        for (i, r) in result.iter().cloned().enumerate() {
            if let Some(label) = names[i] {
                self.globals.id_registry.record(r.0, Cow::Borrowed(label))
            }
            self.globals.graph.record_reaction(r);
            if i < num_non_synthetic {
                if let Some(prev) = prev {
                    // Add an edge that represents that the
                    // previous reaction takes precedence
                    self.globals.graph.reaction_priority(prev, r);
                }
            }
            prev = Some(r);
        }

        self.cur_local += N;
        result
    }

    // register dependencies between components

    pub fn declare_triggers(&mut self, trigger: TriggerId, reaction: GlobalReactionId) -> Result<(), AssemblyError> {
        self.globals.graph.triggers_reaction(trigger, reaction);
        Ok(())
    }

    pub fn effects_port<T: Send>(&mut self, reaction: GlobalReactionId, port: &Port<T>) -> Result<(), AssemblyError> {
        self.effects_instantaneous(reaction, port.get_id())
    }

    // the trigger should be a port or timer
    #[doc(hidden)]
    pub fn effects_instantaneous(&mut self, reaction: GlobalReactionId, trigger: TriggerId) -> Result<(), AssemblyError> {
        self.globals.graph.reaction_effects(reaction, trigger);
        Ok(())
    }

    pub fn declare_uses(&mut self, reaction: GlobalReactionId, trigger: TriggerId) -> Result<(), AssemblyError> {
        self.globals.graph.reaction_uses(reaction, trigger);
        Ok(())
    }

    pub fn bind_ports<T: Send>(&mut self, upstream: &mut Port<T>, downstream: &mut Port<T>) -> Result<(), AssemblyError> {
        crate::bind_ports(upstream, downstream)?;
        self.globals.graph.port_bind(upstream, downstream);
        Ok(())
    }

    /// Create and return a new global id for a new component.
    /// Note: reactions don't share the same namespace as components.
    ///
    /// ### Panics
    ///
    /// See [get_id].
    fn next_comp_id(&mut self, debug_name: Option<Cow<'static, str>>) -> GlobalId {
        let id = GlobalId::new(self.get_id(), self.cur_local);
        if let Some(label) = debug_name {
            self.globals.id_registry.record(id, label);
        }
        self.cur_local += 1;
        id
    }

    /// Register a child reactor.
    pub fn register_reactor<S: ReactorInitializer + 'static>(&mut self, child: S) {
        let vec_id = self.globals.reactors.push(Box::new(child));
        assert_eq!(self.globals.reactors[vec_id].id(), vec_id, "Improper initialization order!");
    }

    /// Assemble a child reactor. The child needs to be registered
    /// using [Self::register_reactor] later.
    #[inline]
    pub fn assemble_sub<S: ReactorInitializer>(&mut self, inst_name: &'static str, args: S::Params) -> Result<S, AssemblyError> {
        let mut sub = AssemblyCtx::new::<S>(&mut self.globals, self.debug.derive::<S>(inst_name));
        S::assemble(args, &mut sub)
    }

    pub(in super) fn new<S: ReactorInitializer>(globals: &'x mut RootAssembler, debug: ReactorDebugInfo) -> Self {
        Self {
            globals,
            reactor_id: None,
            reactions_done: false,
            // this is not zero, so that reaction ids and component ids are disjoint
            cur_local: S::MAX_REACTION_ID,
            debug,
        }
    }
}
