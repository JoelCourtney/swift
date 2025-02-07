// mod activities;
//
// use crate::activities::recharge_potato::RechargePotato;
// use serde::{Deserialize, Serialize};
// use swift::alloc::SendBump;
// use swift::time::Duration;
// use swift::reexports::tokio;
// use swift::{model, ResourceType, Session};
// use swift::history::CopyHistory;
//
// model! {
//     pub struct PotatoSat {
//         battery: f32 = 2.0,
//         temperature: f32 = 5.0,
//
//         mode: OperatingMode = OperatingMode::Nominal
//     }
// }
//
// #[derive(Clone, Serialize, Deserialize, Default, Debug)]
// pub enum OperatingMode {
//     #[default]
//     Nominal,
//     Safe(String),
// }
//
// impl ResourceType for OperatingMode {
//     const PIECEWISE_CONSTANT: bool = true;
//     type History = CopyHistory<OperatingMode>;
// }
//
// #[tokio::main]
// async fn main() {
//     let mut session = Session::<PotatoSat>::default();
//     session.add(Duration(1), RechargePotato { amount: 5 }).await;
//
//     let b = SendBump::new();
//
//     let battery = &*session
//         .op_timelines
//         .battery
//         .last()
//         .run(&b)
//         .await
//         .to_string();
//
//     let b = SendBump::new();
//
//     let temperature = &*session
//         .op_timelines
//         .temperature
//         .last()
//         .run(&b)
//         .await
//         .to_string();
//
//     dbg!(battery, temperature);
// }

use swift::{HasResource, Model, Resource, Time, Timeline, Writer};
use swift::history::{CopyHistory, IndirectHistory};

struct PotatoSat<'h> {
    battery: Timeline<'h, Battery, PotatoSat<'h>>
}

impl Model for PotatoSat<'_> {}

impl<'h> HasResource<'h, Battery> for PotatoSat<'h> {
    fn find_child(&'h self, time: Time) -> &'h (dyn Writer<'h, Battery, Self>) {
        self.battery.last_before(time).1
    }
}

struct Battery;

impl<'h> Resource<'h> for Battery {
    const PIECEWISE_CONSTANT: bool = true;

    type Read = f32;
    type Write = f32;

    type History = CopyHistory<'h, Self>;
}

struct Mode;

impl<'h> Resource<'h> for Mode {
    const PIECEWISE_CONSTANT: bool = true;
    type Read = &'h str;
    type Write = String;
    type History = IndirectHistory<'h, Self>;
}

fn main() {}