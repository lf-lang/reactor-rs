mod reactors;

use reactors::reactor::{Reactor, ProduceReactor};
use crate::reactors::reactor::ConsumeReactor;
use crate::reactors::assembler::Assembler;

fn main() {
    let mut assembler = Assembler::root();
    let mut producer = ProduceReactor::new(&mut assembler);
    let mut consumer = ConsumeReactor::new(&mut assembler);


    assembler.connect(&producer.value, &consumer.input);

    consumer.react_print.fire(&consumer);
    producer.react_incr.fire(&producer);
    consumer.react_print.fire(&consumer);
}
