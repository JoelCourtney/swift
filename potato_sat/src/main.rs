use swift::history::{CopyHistory, DerefHistory};
use swift::{model, Resource};

mod activities;

model! {
    pub PotatoSat {
        battery: Battery,
        mode: Mode
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
    type History = DerefHistory<'h, Self>;
}

fn main() {}
