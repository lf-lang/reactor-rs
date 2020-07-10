mod reactors;

use reactors::reactor::{Reactor, ProduceReactor};
use crate::reactors::reactor::ConsumeReactor;
use crate::reactors::assembler::Assembler;

fn main() {
    let mut assembler = Assembler::new();
    let mut producer = ProduceReactor::new();
    let mut consumer = ConsumeReactor::new(&mut assembler);

    consumer.input.bind(&producer.value);

    consumer.reactions()[0].fire(&consumer); // print

    producer.value.set(42);

    consumer.reactions()[0].fire(&consumer); // print

}
