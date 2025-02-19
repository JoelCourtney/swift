use crate::{Battery, Mode};
use peregrine::impl_activity;
use peregrine::Duration;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct RechargePotato {
    pub amount: u32,
}

impl_activity! { for RechargePotato
    let end = start + Duration::from_hours(1.0);
    @(end) b: Battery -> b, m: Mode {
        b += 4.0;
        m = "help".to_string();
    }
    Duration::ZERO
}
