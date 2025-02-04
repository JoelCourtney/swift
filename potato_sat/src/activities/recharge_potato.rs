use crate::PotatoSat;
use serde::{Deserialize, Serialize};
use swift::duration::{Duration, Durative};
use swift::operation::OperationBundle;
use swift::{impl_activity, Activity, Durative};

#[derive(Serialize, Deserialize, Durative)]
#[duration = "Duration(5)"]
pub struct RechargePotato {
    pub amount: f32,
}

impl_activity! {
    for RechargePotato in PotatoSat {
        end => {
            println!("hi");
            :temperature = -1.0;
            ?:battery += ?temperature + 1.0;
        }
    }
}
