use swift::history::{CopyHistory, DerefHistory};
use swift::{model, Resource};

mod activities;

model! {
    pub PotatoSat {
        battery: Battery,
        mode: Mode
    }
}

#[derive(Debug)]
enum Battery {}

impl<'h> Resource<'h> for Battery {
    const STATIC: bool = true;

    type Read = f32;
    type Write = f32;

    type History = CopyHistory<'h, Self>;
}

#[derive(Debug)]
enum Mode {}

impl<'h> Resource<'h> for Mode {
    const STATIC: bool = true;
    type Read = &'h str;
    type Write = String;
    type History = DerefHistory<'h, Self>;
}

fn main() {}
