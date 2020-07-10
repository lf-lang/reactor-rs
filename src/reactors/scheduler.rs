use std::cmp::{Ordering, Reverse};
use std::cmp::Ordering::{Greater, Less};
use std::sync::Mutex;
use std::time::Duration;

use petgraph::Direction;
use priority_queue::PriorityQueue;

use crate::reactors::action::Action;
use crate::reactors::assembler::{DepGraph, NodeId, Stamped, NodeKind};
use crate::reactors::port::OutPort;
use std::collections::HashSet;
use petgraph::matrix_graph::NodeIndex;

#[derive(Hash, Eq, PartialEq)]
enum Event {
    /// An output port was set -> update its downstream
    PortSet { outport: NodeId },
    Action {
        action: NodeId,
        more_delay: Option<Duration>,
    },
}

#[derive(Ord, PartialOrd, Eq, PartialEq)]
struct LogicalTime(i32);

struct DenseDate {
    logical_time: LogicalTime,
    microstep: i32,
}

impl Ord for DenseDate {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.logical_time.cmp(&other.logical_time) {
            o @ Less | o @ Greater => o,
            Eq => self.microstep.cmp(&other.microstep)
        }
    }
}

type TopoPriority = i32; // TODO

struct Scheduler {
    cur_logical_time: LogicalTime,

    reaction_queue: PriorityQueue<NodeIndex/* */, TopoPriority>,

    queue: PriorityQueue<Event, Reverse<DenseDate>>,
    graph: DepGraph,
}


impl Scheduler {
    // todo thread safety

    fn set<T>(&mut self, port: &Stamped<OutPort<T>>, value: T) {
        port.set(value);

        let downstream = self.graph.neighbors_directed(port.id, Direction::Outgoing);


        for id in downstream {
            let node = self.graph.node_weight(id).unwrap();
            match node.kind() {
                NodeKind::Reaction => {
                    self.reaction_queue.push(Stamped { id, });
                }
                _ => { /* nothing to do */ }
            }
        };
    }

}



