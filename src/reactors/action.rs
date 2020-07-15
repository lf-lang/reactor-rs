use std::time::Duration;

use crate::reactors::id::{GlobalId, Identified};

#[derive(Eq, Hash, Clone, PartialEq, Debug)]
pub struct ActionId {
    min_delay: Duration,
    is_logical: bool,
    global_id: GlobalId,
}

impl ActionId {
    pub(in super) fn new(min_delay: Option<Duration>, id: GlobalId, is_logical: bool) -> Self {
        ActionId {
            min_delay: min_delay.unwrap_or(Duration::new(0, 0)),
            global_id: id,
            is_logical,
        }
    }

    pub fn min_delay(&self) -> Duration {
        self.min_delay
    }

    pub fn is_logical(&self) -> bool {
        self.is_logical
    }
}

impl Identified for ActionId {
    fn global_id(&self) -> &GlobalId {
        &self.global_id
    }
}
