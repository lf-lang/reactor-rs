
/// Ports carry no type information (they're not generic),
/// they're just IDs for the graph
pub enum Port {
    Input(&'static str),
    Output(&'static str),
}
