use std::time::Duration;
use crate::reactors::id::{AssemblyId, GlobalId, Identified};
use crate::reactors::util::Named;

pub struct ActionId {
    min_delay: Duration,
    is_logical: bool,
    global_id: GlobalId,
}

const ZERO_DELAY: Duration = Duration::new(0, 0);

impl ActionId {
    pub(in super) fn new(min_delay: Option<Duration>, id: GlobalId, is_logical: bool) -> Self {
        ActionId { min_delay: min_delay.unwrap_or(ZERO_DELAY), global_id: id, is_logical }
    }
}

impl Identified for ActionId {
    fn global_id(&self) -> &GlobalId {
        &self.global_id
    }
}
