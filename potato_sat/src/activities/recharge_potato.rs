use crate::{battery, mode};
use peregrine::Duration;
use peregrine::impl_activity;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct RechargePotato {
    pub amount: u32,
}

impl_activity! { for RechargePotato
    let end = start + Duration::from_hours(1.0);
    @(end) {
        ref mut: battery += 4.0;
        mut: mode = "help".to_string();
    }
    Duration::ZERO
}
