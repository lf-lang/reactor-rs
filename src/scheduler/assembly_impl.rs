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

use crate::assembly::*;
use crate::scheduler::dependencies::DepGraph;
use crate::*;
use index_vec::{Idx, IndexVec};

use super::{ReactorBox, ReactorVec};

/// Globals shared by all assemblers.
pub(super) struct RootAssembler {
    /// All registered reactors
    pub(super) reactors: IndexVec<ReactorId, Option<ReactorBox<'static>>>,
    /// Dependency graph
    pub(super) graph: DepGraph,
    /// Debug infos
    pub(super) debug_info: DebugInfoRegistry,

    /// Next reactor ID to assign
    reactor_id: ReactorId,
    /// Next trigger ID to assign
    cur_trigger: TriggerId,
}

impl RootAssembler {
    /// Register a reactor into the global data structure that owns them during execution.
    fn register_reactor<R: ReactorInitializer + 'static + Send>(&mut self, child: R) {
        if child.id().index() >= self.reactors.len() {
            self.reactors.resize_with(child.id().index() + 1, || None)
        }
        let prev = self.reactors[child.id()].replace(Box::new(child));
        // this is impossible because we control how we allocate IDs entirely
        debug_assert!(prev.is_none(), "Overwrote a reactor during initialization")
    }

    /// Register reactors into the global data structure that owns them during execution.
    fn register_bank<R: ReactorInitializer + 'static + Send>(&mut self, bank: Vec<R>) {
        for child in bank {
            self.register_reactor(child)
        }
    }

    pub(crate) fn assemble_tree<R: ReactorInitializer + 'static + Send>(
        main_args: R::Params,
    ) -> (ReactorVec<'static>, DepGraph, DebugInfoRegistry) {
        let mut root = RootAssembler::default();
        let assembler = AssemblyCtx::new(&mut root, ReactorDebugInfo::root::<R::Wrapped>());

        let main_reactor = match R::assemble(main_args, assembler) {
            Ok(main) => main,
            Err(e) => std::panic::panic_any(e.lift(&root.debug_info)),
        };
        root.register_reactor(main_reactor);

        let RootAssembler {
            graph,
            reactors,
            debug_info: id_registry,
            ..
        } = root;
        let reactors = reactors
            .into_iter()
            .map(|r| r.expect("Uninitialized reactor!"))
            .collect();
        (reactors, graph, id_registry)
    }
}

impl Default for RootAssembler {
    fn default() -> Self {
        Self {
            reactor_id: ReactorId::new(0),
            graph: DepGraph::new(),
            debug_info: DebugInfoRegistry::new(),
            reactors: Default::default(),
            cur_trigger: TriggerId::FIRST_REGULAR,
        }
    }
}

/// Helper struct to assemble reactors during initialization.
/// One assembly context is used per reactor, they can't be shared.
pub struct AssemblyCtx<'x, S: ReactorInitializer> {
    globals: &'x mut RootAssembler,
    /// Next local ID for components != reactions
    cur_local: LocalReactionId,

    /// Contains debug info for this reactor. Empty after
    /// assemble_self has run, and the info is recorded
    /// into the debug info registry.
    debug: Option<ReactorDebugInfo>,

    _phantom: PhantomData<S>,
}

impl<'x, S: ReactorInitializer> AssemblyCtx<'x, S> {
    fn new(globals: &'x mut RootAssembler, debug: ReactorDebugInfo) -> Self {
        Self {
            globals,
            // this is not zero, so that reaction ids and component ids are disjoint
            cur_local: S::MAX_REACTION_ID,
            debug: Some(debug),
            _phantom: PhantomData,
        }
    }

    /// Note: this needs to be called after all children reactors
    /// have been built, as they're pushed into the global reactor
    /// vec before their parent. So the ID of the parent needs to
    /// be fixed only after all descendants have been built.
    pub fn do_assembly<const N: usize>(
        mut self,
        create_self: impl FnOnce(&mut ComponentCreator<S>, ReactorId) -> Result<S, AssemblyError>,
        num_non_synthetic_reactions: usize,
        reaction_names: [Option<&'static str>; N],
        declare_dependencies: impl FnOnce(
            &mut DependencyDeclarator<S>,
            &mut S,
            [GlobalReactionId; N],
        ) -> AssemblyResult<()>,
    ) -> Result<(Self, S), AssemblyError> {
        // todo when feature(generic_const_exprs) is stabilized,
        //  replace const parameter N with S::MAX_REACTION_ID.index().
        debug_assert_eq!(N, S::MAX_REACTION_ID.index(), "Should initialize all reactions");

        let id = self.globals.reactor_id;
        self.globals.reactor_id = self.globals.reactor_id.plus(1);
        self.globals.debug_info.record_reactor(id, self.debug.take().expect("Can only call assemble_self once"));

        let first_trigger_id = self.globals.cur_trigger;

        let mut ich = create_self(&mut ComponentCreator { assembler: &mut self }, id)?;
        // after creation, globals.cur_trigger has been mutated
        // record proper debug info.
        self.globals.debug_info.set_id_range(id, first_trigger_id..self.globals.cur_trigger);

        // declare dependencies
        let reactions = self.new_reactions(id, num_non_synthetic_reactions, reaction_names);
        declare_dependencies(&mut DependencyDeclarator { assembler: &mut self }, &mut ich, reactions)?;
        Ok((self, ich))
    }

    /// Create N reactions. The first `num_non_synthetic` get
    /// priority edges, as they are taken to be those declared
    /// in LF by the user.
    /// The rest do not have priority edges, and their
    /// implementation must hence have no observable side-effect.
    fn new_reactions<const N: usize>(
        &mut self,
        my_id: ReactorId,
        num_non_synthetic: usize,
        names: [Option<&'static str>; N],
    ) -> [GlobalReactionId; N] {
        assert!(num_non_synthetic <= N);

        let result = array![i => GlobalReactionId::new(my_id, LocalReactionId::from_usize(i)); N];

        let mut prev: Option<GlobalReactionId> = None;
        for (i, r) in result.iter().cloned().enumerate() {
            if let Some(label) = names[i] {
                self.globals
                    .debug_info
                    .record_reaction(r, Cow::Borrowed(label))
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

        self.cur_local = self.cur_local.plus(N);
        result
    }

    /// Assembles a child reactor and makes it available in
    /// the scope of a function.
    #[inline]
    pub fn with_child<Sub: ReactorInitializer + 'static + Send, F>(
        mut self,
        inst_name: &'static str,
        args: Sub::Params,
        action: F,
    ) -> Result<(Self, S), AssemblyError>
    // we can't use impl FnOnce(...) because we want to specify explicit type parameters in the calle
    where
        F: FnOnce(Self, &mut Sub) -> Result<(Self, S), AssemblyError>,
    {
        info!("Assembling {}", inst_name);
        let mut sub = self.assemble_sub(inst_name, None, args)?;
        let (ich, r) = action(self, &mut sub)?;
        info!("Registering {}", inst_name);
        ich.globals.register_reactor(sub);
        Ok((ich, r))
    }

    /// Assembles a bank of children reactor and makes it
    /// available in the scope of a function.
    #[inline]
    pub fn with_child_bank<Sub, A, F>(
        mut self,
        inst_name: &'static str,
        bank_width: usize,
        arg_maker: A,
        action: F,
    ) -> Result<(Self, S), AssemblyError>
    where
        Sub: ReactorInitializer + 'static + Send,
        // we can't use impl Fn(...) because we want to specify explicit type parameters in the calle
        F: FnOnce(Self, &mut Vec<Sub>) -> Result<(Self, S), AssemblyError>,
        A: Fn(/*bank_index:*/ usize) -> Sub::Params,
    {
        info!("Assembling {}", inst_name);

        let mut sub = (0..bank_width)
            .into_iter()
            .map(|i| self.assemble_sub(inst_name, Some(i), arg_maker(i)))
            .collect::<Result<Vec<Sub>, _>>()?;

        let (ich, r) = action(self, &mut sub)?;

        info!("Registering {}", inst_name);
        ich.globals.register_bank(sub);
        Ok((ich, r))
    }

    /// Assemble a child reactor. The child needs to be registered
    /// using [Self::register_reactor] later.
    #[inline(always)]
    fn assemble_sub<Sub: ReactorInitializer>(
        &mut self,
        inst_name: &'static str,
        bank_idx: Option<usize>,
        args: Sub::Params,
    ) -> Result<Sub, AssemblyError> {
        let my_debug = self
            .debug
            .as_ref()
            .expect("should assemble sub-reactors before self");

        let debug_info = match bank_idx {
            None => my_debug.derive::<Sub>(inst_name),
            Some(i) => my_debug.derive_bank_item::<Sub>(inst_name, i),
        };

        let sub = AssemblyCtx::new(&mut self.globals, debug_info);
        Sub::assemble(args, sub)
    }
}

pub struct DependencyDeclarator<'a, 'x, S: ReactorInitializer> {
    assembler: &'a mut AssemblyCtx<'x, S>,
}

impl<S: ReactorInitializer> DependencyDeclarator<'_, '_, S> {
    pub fn declare_triggers(
        &mut self,
        trigger: TriggerId,
        reaction: GlobalReactionId,
    ) -> AssemblyResult<()> {
        self.graph().triggers_reaction(trigger, reaction);
        Ok(())
    }

    pub fn effects_port<T: Sync>(
        &mut self,
        reaction: GlobalReactionId,
        port: &Port<T>,
    ) -> AssemblyResult<()> {
        self.effects_instantaneous(reaction, port.get_id())
    }

    pub fn effects_bank<T: Sync>(
        &mut self,
        reaction: GlobalReactionId,
        port: &PortBank<T>,
    ) -> AssemblyResult<()> {
        self.effects_instantaneous(reaction, port.get_id())
    }

    #[doc(hidden)] // used by synthesized timer reactions
    pub fn effects_timer(
        &mut self,
        reaction: GlobalReactionId,
        timer: &Timer,
    ) -> AssemblyResult<()> {
        self.effects_instantaneous(reaction, timer.get_id())
    }

    fn effects_instantaneous(
        &mut self,
        reaction: GlobalReactionId,
        trigger: TriggerId,
    ) -> AssemblyResult<()> {
        self.graph().reaction_effects(reaction, trigger);
        Ok(())
    }

    pub fn declare_uses(
        &mut self,
        reaction: GlobalReactionId,
        trigger: TriggerId,
    ) -> AssemblyResult<()> {
        self.graph().reaction_uses(reaction, trigger);
        Ok(())
    }

    /// Bind two ports together.
    #[inline]
    pub fn bind_ports<T: Sync>(
        &mut self,
        upstream: &mut Port<T>,
        downstream: &mut Port<T>,
    ) -> AssemblyResult<()> {
        crate::bind_ports(upstream, downstream)?;
        self.graph().port_bind(upstream, downstream);
        Ok(())
    }

    /// Bind the ports of the upstream to those of the downstream,
    /// as if zipping both iterators.
    /// todo this will just throw away bindings if both iterators are not of the same size
    ///  normally this should be reported by LFC as a warning, maybe we should implement the same thing here
    #[inline]
    pub fn bind_ports_zip<'a, T: Sync + 'a>(
        &mut self,
        upstream: impl Iterator<Item = &'a mut Port<T>>,
        downstream: impl Iterator<Item = &'a mut Port<T>>,
    ) -> AssemblyResult<()> {
        for (upstream, downstream) in upstream.zip(downstream) {
            self.bind_ports(upstream, downstream)?;
        }
        Ok(())
    }

    #[inline]
    fn graph(&mut self) -> &mut DepGraph {
        &mut self.assembler.globals.graph
    }
}

/// Creates the components of a reactor.
pub struct ComponentCreator<'a, 'x, S: ReactorInitializer> {
    assembler: &'a mut AssemblyCtx<'x, S>,
}

impl<S: ReactorInitializer> ComponentCreator<'_, '_, S> {
    pub fn new_port<T: Sync>(&mut self, lf_name: &'static str, is_input: bool) -> Port<T> {
        self.new_port_impl(Cow::Borrowed(lf_name), is_input)
    }

    fn new_port_impl<T: Sync>(&mut self, lf_name: Cow<'static, str>, is_input: bool) -> Port<T> {
        let id = self.next_comp_id(Some(lf_name));
        self.graph().record_port(id);
        Port::new(id, is_input)
    }

    pub fn new_port_bank<T: Sync>(
        &mut self,
        lf_name: &'static str,
        is_input: bool,
        len: usize,
    ) -> Result<PortBank<T>, AssemblyError> {
        let bank_id = self.next_comp_id(Some(Cow::Borrowed(lf_name)));
        self.graph().record_port_bank(bank_id, len)?;
        Ok(PortBank::new(
            (0..len)
                .into_iter()
                .map(|i| self.new_port_bank_component(lf_name, is_input, bank_id, i))
                .collect(),
            bank_id,
        ))
    }

    fn new_port_bank_component<T: Sync>(
        &mut self,
        lf_name: &'static str,
        is_input: bool,
        bank_id: TriggerId,
        index: usize,
    ) -> Port<T> {
        let channel_id = self.next_comp_id(Some(Cow::Owned(format!("{}[{}]", lf_name, index))));
        self.graph().record_port_bank_component(bank_id, channel_id);
        Port::new(channel_id, is_input)
    }

    pub fn new_logical_action<T: Sync>(
        &mut self,
        lf_name: &'static str,
        min_delay: Option<Duration>,
    ) -> LogicalAction<T> {
        let id = self.next_comp_id(Some(Cow::Borrowed(lf_name)));
        self.graph().record_laction(id);
        LogicalAction::new(id, min_delay)
    }

    pub fn new_physical_action<T: Sync>(
        &mut self,
        lf_name: &'static str,
        min_delay: Option<Duration>,
    ) -> PhysicalActionRef<T> {
        let id = self.next_comp_id(Some(Cow::Borrowed(lf_name)));
        self.graph().record_paction(id);
        PhysicalActionRef::new(id, min_delay)
    }

    pub fn new_timer(
        &mut self,
        lf_name: &'static str,
        offset: Duration,
        period: Duration,
    ) -> Timer {
        let id = self.next_comp_id(Some(Cow::Borrowed(lf_name)));
        self.graph().record_timer(id);
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
            self.assembler.globals.debug_info.record_trigger(id, label);
        }
        self.assembler.globals.cur_trigger = self
            .assembler
            .globals
            .cur_trigger
            .next()
            .expect("Overflow while allocating ID");
        id
    }

    #[inline]
    fn graph(&mut self) -> &mut DepGraph {
        &mut self.assembler.globals.graph
    }
}

/// Iterates a bank, produces an `Iterator<Item=&mut Port<_>>`.
/// Does not explicitly borrow the bank, which is unsafe, but
/// we trust the code generator to fail if a port is both on
/// the LHS and RHS of a connection.
///
/// This is necessary
///
#[macro_export]
macro_rules! unsafe_iter_bank {
    // the field is not a multiport
    ($bank:ident # $field_name:ident) => {{
        let __ptr = $bank.as_mut_ptr();
        (0..$bank.len())
            .into_iter()
            .map(move |i| unsafe { &mut (*__ptr.add(i)).$field_name })
    }};
    // the field is a multiport, we select a single index
    ($bank:ident # $field_name:ident[$idx:expr]) => {{
        let __ptr = $bank.as_mut_ptr();
        (0..$bank.len())
            .into_iter()
            .map(move |i| unsafe { &mut (*__ptr.add(i)).$field_name[$idx] })
    }};
    // the field is a multiport, we select all of them
    ($bank:ident # ($field_name:ident)+) => {{
        let __ptr = $bank.as_mut_ptr();
        (0..$bank.len())
            .into_iter()
            .map(move |i| unsafe { &mut (*__ptr.add(i)) })
            .flat_map(|a| a.$field_name.iter_mut())
    }};
}
