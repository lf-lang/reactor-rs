
pub struct InPort<'a> {
    pub name: &'static str,
    binding: Option<Box<&'a OutPort>>,
}

impl<'a> InPort<'a> {
    pub fn bind(&mut self, out: Box<&'a OutPort>) {
        self.binding = Some(out)
    }
}


/// Output ports are immutable, meaning,
/// it's ok to share them in the binding of input ports,
/// as the borrows will only ever be immutable
pub struct OutPort {
    pub name: &'static str,
}

/// Ports carry no type information (they're not generic),
/// they're just topological guides
pub enum Port<'a> {
    Input(InPort<'a>),
    Output(OutPort),
}

impl<'a> Port<'a> {
    pub fn new_input(name: &'static str) -> Port {
        // let binding: Option<Box<&'_ >> = None
        Port::Input(InPort { name, binding: None })
    }

    pub fn new_output(name: &'static str) -> Port {
        Port::Output(OutPort { name })
    }
}

