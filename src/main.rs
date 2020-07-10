mod reactors;

use reactors::reactor::{Reactor, ProduceReactor};
use crate::reactors::reactor::ConsumeReactor;
use crate::reactors::assembler::Assembler;

fn main() {
    let mut assembler = Assembler::new();
    let mut producer = ProduceReactor::new(&mut assembler);
    let mut consumer = ConsumeReactor::new(&mut assembler);


    assembler.connect(&producer.value, &consumer.input);

    consumer.reactions()[0].fire(&consumer); // print

    producer.value.data.set(42);

    consumer.reactions()[0].fire(&consumer); // print
}
