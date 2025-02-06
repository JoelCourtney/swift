use crate::PotatoSat;
use serde::{Deserialize, Serialize};
use swift::{impl_activity, op, Activity, Duration, Durative, Time};
use swift::operation::GroundedOperationBundle;

#[derive(Serialize, Deserialize, Durative, Clone)]
#[duration = "Duration(5)"]
pub struct RechargePotato {
    pub amount: u32,
}

// impl_activity! {
//     for RechargePotato in PotatoSat {
//         end => {
//             println!("hi");
//             ?:battery += args.amount as f32;
//             :temperature = 10.0;
//         }
//     }
// }

op!(battery -> battery, temperature {
    println!("hi");
    battery += args.amount as f32;
    temperature = 10;
});

impl Activity<PotatoSat> for RechargePotato {
    fn run(&self, start: Time) -> Vec<GroundedOperationBundle<Self::Model>> {

        todo!()
    }


}
