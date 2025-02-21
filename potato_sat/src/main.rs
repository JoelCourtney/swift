use peregrine::{model, resource};

mod activities;

model! {
    pub PotatoSat(battery, mode)
}

resource!(battery: f32);
resource!(ref mode: String);

fn main() {}
