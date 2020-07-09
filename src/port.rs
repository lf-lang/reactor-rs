use std::rc::Rc;
use std::cell::RefCell;


struct InPort<'a> {
    name: &'static str,
    binding: Option<Box<&'a OutPort>>,
}


/// Output ports are immutable, meaning,
/// it's ok to share them in the binding of input ports,
/// as the borrows will only ever be immutable
struct OutPort {
    name: &'static str,
}

/// Ports carry no type information (they're not generic),
/// they're just topological guides
enum Port<'a> {
    Input(InPort<'a>),
    Output(OutPort),
}

impl<'a> Port<'a> {
    fn new_input(name: &'static str) -> Port {
        // let binding: Option<Box<&'_ >> = None
        Port::Input(InPort { name, binding: None })
    }

    fn new_output(name: &'static str) -> Port {
        Port::Output(OutPort { name })
    }
}

impl<'a> InPort<'a> {
    fn bind(&mut self, out: Box<&'a OutPort>) {
        self.binding = Some(out)
    }
}

