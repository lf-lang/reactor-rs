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
use std::marker::PhantomData;

use crate::*;
use crate::scheduler::dependencies::DepGraph;

use super::ReactorVec;

pub(in super) struct RootAssembler {
    /// ID of the next reactor to assign
    reactor_id: ReactorId,
    /// All registered reactors
    pub(in super) reactors: ReactorVec<'static>,
    /// Dependency graph
    pub(in super) graph: DepGraph,
    pub(in super) id_registry: DebugInfoRegistry,
    /// Next trigger ID to assign
    cur_trigger: TriggerId,
}

impl Default for RootAssembler {
    fn default() -> Self {
        Self {
            reactor_id: ReactorId::new(0),
            graph: DepGraph::new(),
            id_registry: DebugInfoRegistry::new(),
            reactors: Default::default(),
            cur_trigger: TriggerId::FIRST_REGULAR
        }
    }
}


/// Helper struct to assemble reactors during initialization.
/// One assembly context is used per reactor, they can't be shared.
pub struct AssemblyCtx<'x, S: ReactorInitializer> {
    globals: &'x mut RootAssembler,
    /// Constant id of the reactor currently being built.
    reactor_id: Option<ReactorId>,
    /// Next local ID for components != reactions
    cur_local: LocalReactionId,
    /// Whether reactions have already been created
    reactions_done: bool,

    /// Contains debug info for this reactor. Empty after
    /// assemble_self has run, and the info is recorded
    /// into the debug info registry.
    debug: Option<ReactorDebugInfo>,

    _phantom: PhantomData<&'x S>,
}

impl<'x, S: ReactorInitializer> AssemblyCtx<'x, S> {
    /// The ID of the reactor being built.
    ///
    /// ### Panics
    /// If fix_cur_id has not been called.
    // todo remove this
    pub fn get_id(&self) -> ReactorId {
        self.reactor_id.unwrap_or_else(|| panic!("fix_cur_id has not been called"))
    }

    /// Note: this needs to be called after all children reactors
    /// have been built, as they're pushed into the global reactor
    /// vec before their parent. So the ID of the parent needs to
    /// be fixed only after all descendants have been built.
    pub fn assemble_self(&mut self, creation_fun: impl FnOnce(&mut ComponentCreator<S>, ReactorId) -> S) -> S {
        let id = self.globals.reactor_id;
        self.globals.reactor_id += 1;
        self.reactor_id = Some(id);
        self.globals.id_registry.record_reactor(id, self.debug.take().expect("Can only call assemble_self once"));

        let first_trigger_id = self.globals.cur_trigger;

        let result = creation_fun(&mut ComponentCreator { assembler: self }, id);

        self.globals.id_registry.set_id_range(id, first_trigger_id..self.globals.cur_trigger);

        result
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
                self.globals.id_registry.record_reaction(r, Cow::Borrowed(label))
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

    pub fn effects_port<T: Sync>(&mut self, reaction: GlobalReactionId, port: &Port<T>) -> Result<(), AssemblyError> {
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

    pub fn bind_ports<T: Sync>(&mut self, upstream: &mut Port<T>, downstream: &mut Port<T>) -> Result<(), AssemblyError> {
        crate::bind_ports(upstream, downstream)?;
        self.globals.graph.port_bind(upstream, downstream);
        Ok(())
    }


    /// Register a child reactor.
    pub fn register_reactor<Sub: ReactorInitializer + 'static>(&mut self, child: Sub) {
        let vec_id = self.globals.reactors.push(Box::new(child));
        assert_eq!(self.globals.reactors[vec_id].id(), vec_id, "Improper initialization order!");
    }

    /// Assemble a child reactor. The child needs to be registered
    /// using [Self::register_reactor] later.
    #[inline]
    pub fn assemble_sub<Sub: ReactorInitializer>(&mut self, inst_name: &'static str, args: Sub::Params) -> Result<Sub, AssemblyError> {
        let my_debug = self.debug.as_ref().expect("should assemble sub-reactors before self");
        let mut sub = AssemblyCtx::new(&mut self.globals, my_debug.derive::<Sub>(inst_name));
        Sub::assemble(args, &mut sub)
    }

    pub(super) fn new(globals: &'x mut RootAssembler, debug: ReactorDebugInfo) -> Self {
        Self {
            globals,
            reactor_id: None,
            reactions_done: false,
            // this is not zero, so that reaction ids and component ids are disjoint
            cur_local: S::MAX_REACTION_ID,
            debug: Some(debug),
            _phantom: PhantomData,
        }
    }
}


pub struct ComponentCreator<'a, 'x, S: ReactorInitializer> {
    assembler: &'a mut AssemblyCtx<'x, S>,
}

impl<S: ReactorInitializer> ComponentCreator<'_, '_, S> {
    pub fn new_port<T: Sync>(&mut self, lf_name: &'static str, is_input: bool) -> Port<T> {
        self.new_port_impl(Cow::Borrowed(lf_name), is_input)
    }

    fn new_port_impl<T: Sync>(&mut self, lf_name: Cow<'static, str>, is_input: bool) -> Port<T> {
        let id = self.next_comp_id(Some(lf_name));
        self.assembler.globals.graph.record_port(id);
        Port::new(id, is_input)
    }

    // not sure if this will ever serve
    pub fn new_port_bank_const<T: Sync, const N: usize>(&mut self, lf_name: &'static str, is_input: bool) -> [Port<T>; N] {
        array![i => self.new_port_bank_component(lf_name, is_input, i); N]
    }

    pub fn new_port_bank<T: Sync>(&mut self, lf_name: &'static str, is_input: bool, len: usize) -> MultiPort<T> {
        let id = self.next_comp_id(Some(Cow::Borrowed(lf_name)));
        self.assembler.globals.graph.record_multiport(id, len);
        MultiPort::new(
            (0..len).into_iter().map(|i| self.new_port_bank_component(lf_name, is_input, i)).collect(),
            id,
        )
    }

    fn new_port_bank_component<T: Sync>(&mut self, lf_name: &'static str, is_input: bool, index: usize) -> Port<T> {
        let label = Cow::Owned(format!("{}[{}]", lf_name, index));
        self.new_port_impl::<T>(label, is_input)
    }

    pub fn new_logical_action<T: Sync>(&mut self,
                                       lf_name: &'static str,
                                       min_delay: Option<Duration>) -> LogicalAction<T> {
        let id = self.next_comp_id(Some(Cow::Borrowed(lf_name)));
        self.assembler.globals.graph.record_laction(id);
        LogicalAction::new(id, min_delay)
    }

    pub fn new_physical_action<T: Sync>(&mut self,
                                        lf_name: &'static str,
                                        min_delay: Option<Duration>) -> PhysicalActionRef<T> {
        let id = self.next_comp_id(Some(Cow::Borrowed(lf_name)));
        self.assembler.globals.graph.record_paction(id);
        PhysicalActionRef::new(id, min_delay)
    }

    pub fn new_timer(&mut self, lf_name: &'static str, offset: Duration, period: Duration) -> Timer {
        let id = self.next_comp_id(Some(Cow::Borrowed(lf_name)));
        self.assembler.globals.graph.record_timer(id);
        Timer::new(id, offset, period)
    }

    /// Create and return a new global id for a new component.
    /// Note: reactions don't share the same namespace as components.
    ///
    /// ### Panics
    ///
    /// See [get_id].
    fn next_comp_id(&mut self, debug_name: Option<Cow<'static, str>>) -> TriggerId {
        let id = self.assembler.globals.cur_trigger;
        if let Some(label) = debug_name {
            self.assembler.globals.id_registry.record_trigger(id, label);
        }
        self.assembler.globals.cur_trigger = self.assembler.globals.cur_trigger.next().expect("Overflow while allocating ID");
        id
    }
}
