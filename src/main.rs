mod reactors;

use reactors::reactor::{ProduceReactor, ConsumeReactor};
use reactors::assembler::Assembler;

fn main() {
    let mut assembler = Assembler::new();
    let producer = ProduceReactor::new(&mut assembler);
    let consumer = ConsumeReactor::new(&mut assembler);


    assembler.connect(&producer.value, &consumer.input);

    consumer.react_print.fire(&consumer);
    producer.react_incr.fire(&producer);
    consumer.react_print.fire(&consumer);
}
