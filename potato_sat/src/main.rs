mod activities;

use crate::activities::recharge_potato::RechargePotato;
use serde::{Deserialize, Serialize};
use swift::duration::Duration;
use swift::reexports::tokio;
use swift::{model, Resource, Session};

model! {
    pub struct PotatoSat {
        battery: f32 = 2.0,
        temperature: f32 = 5.0,

        mode: OperatingMode = OperatingMode::Nominal
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub enum OperatingMode {
    #[default]
    Nominal,
    Safe(String),
}

impl Resource for OperatingMode {
    const PIECEWISE_CONSTANT: bool = true;
}

#[tokio::main]
async fn main() {
    let mut session = Session::<PotatoSat>::default();
    session
        .add(Duration(1), RechargePotato { amount: 5 })
        .await;

    let battery = &*session
        .op_timelines
        .battery
        .last()
        .run(&session.history)
        .await
        .to_string();

    let temperature = &*session
        .op_timelines
        .temperature
        .last()
        .run(&session.history)
        .await
        .to_string();

    dbg!(battery, temperature);
}


