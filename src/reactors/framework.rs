use std::fmt::Debug;
use std::time::Duration;


pub enum PortId<T> {
    Input { name: &'static str },
    Output { name: &'static str },
}

pub struct InputPortId<T> {}

pub struct OutputPortId<T> {}

pub struct ActionId {}

/// A type whose instances have statically known names
trait Named {
    fn name(&self) -> &'static str;
}

/// A type that can list all its instances
pub trait Enumerated {
    fn list() -> Vec<Self>;
}

/// Describes the structure of a reactor.
///
/// Instances are created by cooperating with an [Assembler].
/// Sub-components of the reactor (ports, sub-reactors, etc)
/// are created by the assembler and should be stored in
/// instances. They're immutable.
///
/// Mutable state variables are split into a [State] associated
/// type, that is managed by the framework.
pub trait Reactor {
    /// Enumerates the reactions available for this reactor.
    /// This is used as input to the [react] function.
    type ReactionId: Ord + Eq + Enumerated + Named;

    /// The type for the internal state of the reactor.
    ///
    /// The self instance should not contain the internal state variables.
    /// It couldn't use it anyway, since the [react] method take a `self`
    /// argument as an immutable reference.
    ///
    /// Use `()` for a stateless reactor.
    type State;

    /// Produce the initial state. This is passed by reference
    /// to the [react] function.
    fn initial_state() -> Self::State;

    /// Initializes the structure of this reactor.
    /// This will create subcomponents and link them using the [Assembler].
    ///
    /// The returned instance is wrapped into a [RunnableReactor] for execution.
    fn assemble(assembler: &mut dyn Assembler<Self>) -> Self;

    /// Execute a reaction of this reactor.
    fn react(
        // This is the assembled reactor. It's immutable in this method
        reactor: &RunnableReactor<Self>,
        // A mutable reference to the internal reactor state
        state: &mut Self::State,
        // ID of the reaction to execute
        reaction_id: Self::ReactionId,
        // Scheduler instance, that can make the reaction affect the event queue
        scheduler: &mut dyn Scheduler,
    );
}


/// The output of the assembly of a reactor.
pub struct RunnableReactor<R: Reactor> {
    me: R
}


/// Assembles a reactor.
pub trait Assembler<R: Reactor> {
    /*
     * These methods create new subcomponents, they're supposed
     * to be stored on the struct of the reactor.
     */

    fn new_output_port<T>(&mut self, name: &str) -> PortId<T>;
    fn new_input_port<T>(&mut self, name: &str) -> PortId<T>;
    fn new_action(&mut self, name: &str, delay: Option<Duration>, is_physical: bool) -> ActionId;

    /// Assembles a subreactor. After this, the ports of the subreactor
    /// may be used in some connections, see [reaction_uses], [reaction_affects]
    fn new_subreactor<S: Reactor>(&mut self, name: &str) -> RunnableReactor<S>;

    /*
     * These methods record dependencies between components.
     *
     * These 2 are trigger dependencies, they may be cyclic (but have delays)
     */

    /// Record that an action triggers the given reaction
    ///
    /// Validity: the action ID was created by this assembler
    fn action_triggers(&mut self, port: ActionId, reaction_id: R::ReactionId);


    /// Record that the given reaction may schedule the action for (future)? execution
    ///
    /// Validity: the action ID was created by this assembler
    fn reaction_schedules(&mut self, reaction_id: R::ReactionId, action: ActionId);

    /*
     * The remaining ones are data-flow dependencies, i.e. relevant for the priority graph, which is a DAG
     */

    /// Binds the values of the given two ports. Every value set
    /// to the upstream port will be reflected in the downstream port.
    ///
    /// Validity: either
    ///  1. upstream is an input port of this reactor, and either
    ///   1.i   downstream is an input port of a direct sub-reactor
    ///   1.ii  downstream is an output port of this reactor
    ///  2. upstream is an output port of a direct sub-reactor, and either
    ///   2.i  downstream is an input port of another sub-reactor
    ///   2.ii downstream is an output port of this reactor
    fn bind_ports<T>(&mut self, upstream: PortId<T>, downstream: PortId<T>);

    /// Record that the reaction depends on the value of the given port
    ///
    /// Validity: either
    ///  1. the port is an input port of this reactor
    ///  2. the port is an output port of a direct sub-reactor
    fn reaction_uses<T>(&mut self, reaction_id: R::ReactionId, port: PortId<T>);


    /// Record that the given reaction may set the value of the port
    ///
    ///  1. the port is an output port of this reactor
    ///  2. the port is an input port of a direct sub-reactor
    fn reaction_affects<T>(&mut self, reaction_id: R::ReactionId, port: PortId<T>);
}


/// Schedules actions during the execution of a reaction.
///
/// A scheduler must know which reaction is currently executing,
/// and to which reactor it belongs, in order to validate its
/// input.
pub trait Scheduler {
    /// Sets the value of the given output port. The change
    /// is visible at the same logical time, ie the value
    /// propagates immediately. This may hence schedule more
    /// reactions that should execute on the same logical
    /// step.
    ///
    /// Validity: the port belongs to the reactor whose reaction is being executed
    ///
    fn set_port<T>(&mut self, port: OutputPortId<T>, value: T);

    /// Schedule an action to run after its own implicit time delay,
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    ///
    fn schedule_action(&mut self, action: ActionId, additional_delay: Option<Duration>);
}
