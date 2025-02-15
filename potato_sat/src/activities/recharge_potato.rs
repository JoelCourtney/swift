use crate::{Battery, Mode};
use serde::{Deserialize, Serialize};
use swift::activity;
use swift::Duration;

#[derive(Serialize, Deserialize, Clone)]
pub struct RechargePotato {
    pub amount: u32,
}

activity! {
    for RechargePotato {
        let end = start + Duration::from_hours(1.0);
        @(end) b: Battery -> b, m: Mode {
            b += 4.0;
            m = "help".to_string();
        }
        println!("asdf");
    }
}
