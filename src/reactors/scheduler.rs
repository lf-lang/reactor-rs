use std::cell::{Ref, RefMut};
use std::cmp::Reverse;
use std::fmt::Debug;
use std::ops::Deref;
use std::rc::Rc;
use std::time::{Duration, Instant};

use priority_queue::PriorityQueue;

use crate::reactors::{ActionId, Port};
use crate::reactors::flowgraph::Schedulable;
use crate::reactors::id::{Identified, PortId, ReactionId};
use crate::reactors::reaction::ClosedReaction;

type MicroStep = u32;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash)]
struct LogicalTime {
    instant: Instant,
    microstep: MicroStep,
}

impl Default for LogicalTime {
    fn default() -> Self {
        Self { instant: Instant::now(), microstep: 0 }
    }
}

#[derive(Eq, PartialEq, Hash)]
enum Event<'g> {
    ReactionExecute { at: LogicalTime, reaction: Rc<ClosedReaction<'g>> },
    ReactionSchedule { min_at: LogicalTime, reaction: Rc<ClosedReaction<'g>> },
}

/// Directs execution of the whole reactor graph.
pub struct Scheduler<'g> {
    schedulable: Schedulable<'g>,

    cur_logical_time: LogicalTime,
    micro_step: MicroStep,
    queue: PriorityQueue<Event<'g>, Reverse<LogicalTime>>,
}

impl<'g> Scheduler<'g> {
    // todo logging

    pub(in super) fn new(schedulable: Schedulable<'g>) -> Self {
        Scheduler {
            schedulable,
            cur_logical_time: <_>::default(),
            micro_step: 0,
            queue: PriorityQueue::new(),
        }
    }

    pub fn launch(&mut self, startup_action: &ActionId) {
        self.enqueue_action(startup_action, None);
        while !self.queue.is_empty() {
            self.step()
        }
    }

    fn step(&mut self) {
        if let Some((event, Reverse(time))) = self.queue.pop() {
            let reaction = match event {
                Event::ReactionExecute { reaction, .. } => reaction,
                Event::ReactionSchedule { reaction, .. } => reaction
            };

            self.catch_up_physical_time(time);
            self.cur_logical_time = time;

            let mut ctx = ReactionCtx {
                scheduler: self,
                reaction_id: ReactionId((*reaction).global_id().clone()),
            };
            reaction.fire(&mut ctx)
        }
    }

    fn catch_up_physical_time(&mut self, up_to_time: LogicalTime) {
        let now = Instant::now();
        if now < up_to_time.instant {
            std::thread::sleep(up_to_time.instant - now);
        }
    }

    fn enqueue_port(&mut self, port_id: &PortId) {
        // todo possibly, reactions must be scheduled at most once per logical time step?
        for reaction in self.schedulable.get_downstream_reactions(port_id) {
            let evt = Event::ReactionExecute { at: self.cur_logical_time, reaction: Rc::clone(reaction) };
            self.queue.push(evt, Reverse(self.cur_logical_time));
        }
    }

    fn enqueue_action(&mut self, action_id: &ActionId, additional_delay: Option<Duration>) {
        let min_delay = action_id.min_delay() + additional_delay.unwrap_or(Duration::from_secs(0));

        let mut instant = self.cur_logical_time.instant + min_delay;
        if !action_id.is_logical() {
            // physical actions are adjusted to physical time if needed
            instant = Instant::max(instant, Instant::now());
        }

        // note that the microstep is global, doesn't really matter though
        self.micro_step += 1;
        let eta = LogicalTime {
            instant,
            microstep: self.micro_step,
        };

        for reaction in self.schedulable.get_triggered_reactions(action_id) {
            let evt = Event::ReactionSchedule { min_at: eta, reaction: Rc::clone(reaction) };
            self.queue.push(evt, Reverse(eta));
        }
    }
}


/// This is the context in which a reaction executes. Its API
/// allows mutating the event queue of the scheduler. Only the
/// interactions declared at assembly time are allowed.
///
pub struct ReactionCtx<'a, 'g> {
    scheduler: &'a mut Scheduler<'g>,
    reaction_id: ReactionId,
}

impl<'a, 'g> ReactionCtx<'a, 'g> {
    /// Get the value of a port at this time.
    ///
    /// # Panics
    ///
    /// If the reaction being executed has not declared its
    /// dependency on the given port ([reaction_uses](super::Assembler::reaction_uses)).
    ///
    pub fn get_port<T>(&self, port: &Port<T>) -> T where Self: Sized, T: Copy {
        self.assert_has_read_access(port);

        port.copy_get()
    }

    /// Sets the value of the given output port. The change
    /// is visible at the same logical time, ie the value
    /// propagates immediately. This may hence schedule more
    /// reactions that should execute on the same logical
    /// step.
    ///
    /// # Panics
    ///
    /// If the reaction being executed has not declared its
    /// dependency on the given port ([reaction_affects](super::Assembler::reaction_affects)).
    ///
    pub fn set_port<T>(&mut self, port: &Port<T>, value: T) where Self: Sized, T: Copy {
        self.assert_has_write_access(port);

        port.set(value);

        self.scheduler.enqueue_port(port.port_id());
    }

    /// Executes an action that uses an immutable reference to
    /// the internal value of a port.
    /// TODO the enqueue_port actions should be wrapped around the ref.
    ///  That way we can enqueue only if it was set.
    ///  Then describes these effects
    ///
    /// If the type of the port implements [Copy], you can instead use [Self::set_port].
    ///
    /// # Panics
    ///
    /// If the reaction being executed has not declared its
    /// dependency on the given port ([reaction_affects](super::Assembler::reaction_affects)).
    ///
    ///
    pub fn with_port_mut<T, A>(&mut self, port: &Port<T>, action: A)
        where A: FnOnce(&mut ReactionCtx<'a, 'g>, RefMut<T>) {
        self.assert_has_write_access(port);
        let rcell = port.get_mut();
        let refmut = rcell.borrow_mut();
        self.scheduler.enqueue_port(port.port_id());

        action.call_once((self, refmut));
    }


    /// Executes an action that uses a mutable reference to
    /// the internal value of a port. If the type of the port
    /// implements [Copy], you can instead use [Self::get_port].
    ///
    /// # Panics
    ///
    /// If the reaction being executed has not declared its
    /// dependency on the given port ([reaction_affects](super::Assembler::reaction_uses)).
    ///
    /// If the port was already borrowed mutably in this context.
    ///
    pub fn with_port_ref<T, A>(&mut self, port: &Port<T>, action: A)
        where A: FnOnce(&mut ReactionCtx<'a, 'g>, Ref<T>) {
        self.assert_has_read_access(port);
        let rc = port.get_mut();
        let rcell = rc.deref();
        let r: Ref<T> = rcell.borrow();

        action.call_once((self, r));
    }


    fn assert_has_read_access<T>(&self, port: &Port<T>) {
        assert!(self.scheduler.schedulable.get_allowed_reads(&self.reaction_id).contains(port.port_id()),
                "Forbidden read on port {} by reaction {}. Declare the dependency explicitly during assembly",
                port.global_id(), self.reaction_id.global_id()
        );
    }

    fn assert_has_write_access<T>(&mut self, port: &Port<T>) {
        assert!(self.scheduler.schedulable.get_allowed_writes(&self.reaction_id).contains(port.port_id()),
                "Forbidden read on port {} by reaction {}. Declare the dependency explicitly during assembly",
                port.global_id(), self.reaction_id.global_id()
        );
    }

    /// Schedule an action to run after its own implicit time delay,
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    ///
    /// # Panics
    ///
    /// If the reaction being executed has not declared its
    /// dependency on the given action ([reaction_schedules](super::Assembler::reaction_schedules)).
    pub fn schedule_action(&mut self, action: &ActionId, offset: Option<Duration>) {
        assert!(self.scheduler.schedulable.get_allowed_schedules(&self.reaction_id).contains(action),
                "Forbidden schedule call on action {} by reaction {}. Declare the dependency explicitly during assembly",
                action.global_id(), self.reaction_id.global_id()
        );

        self.scheduler.enqueue_action(action, offset)
    }
}
