use crate::PotatoSat;
use serde::{Deserialize, Serialize};
use swift::{impl_activity, Activity, Duration, Durative};

#[derive(Serialize, Deserialize, Durative, Clone)]
#[duration = "Duration(5)"]
pub struct RechargePotato {
    pub amount: u32,
}

impl_activity! {
    for RechargePotato in PotatoSat {
        end => {
            println!("hi");
            ?:battery += args.amount as f32;
            :temperature = 10.0;
        }
    }
}
