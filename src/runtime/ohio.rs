use std::time::Instant;
use std::fmt::Display;

// A struct that defines two fields
struct LogicalTime { instant: Instant, step: i32 }

struct URL;

trait Steppable {
    type NextStep;                    // type declaration
    fn step(&self) -> Self::NextStep; // method declaration
}

impl Steppable for LogicalTime {
    type NextStep = Self;

    fn step(&self) -> Self::NextStep {
        self.next_step()
    }
}

impl LogicalTime { // impl block
fn println(&self) {
    // println!("Logical time(instant={}, step={})", self.instant, self.step);
}

    // method declaration
    fn next_step(&self) -> LogicalTime {
        // last expression is returned
        LogicalTime {
            instant: self.instant, // implicit copy of instant
            step: self.step + 1,
        }
    }
}

// An enum with two variants
enum Request {
    Put { id: URL, content: String },
    Get { id: URL },
}

fn fun(time: LogicalTime) {
    let time2: LogicalTime = time; // move time into time2

    // time.println();  // error, time was moved
    time2.println(); // ok
}
