mod toys;
mod reactors;

use reactors::reactor::{Reactor, ProduceReactor};
use toys::zipped_tree::{Node, NodeZipper};
use crate::reactors::reactor::ConsumeReactor;

fn main() {
    let mut producer = ProduceReactor::new();
    let mut consumer = ConsumeReactor::new();

    consumer.input.bind(&producer.value);

    consumer.emit();

    producer.value.set(42);

    consumer.emit();


    //
    // let mut root = Node::new("");
    // let mut c0 = Node::new("0");
    // let mut c00 = Node::new("00");
    // c0.add_child(c00);
    //
    // let mut c1 = Node::new("1");
    //
    // root.add_child(c0);
    // root.add_child(c1);
    //
    // println!("{:?}", root);
    //
    // let mut zip = root.zipper();
    // zip = zip.child(1);
    // zip.node = Node::new("42");
    // zip = zip.parent();
    // zip = zip.child(0).child(0);
    // zip.node.data = "4";
    // root = zip.finish();
    // println!("{:?}", root);
}
