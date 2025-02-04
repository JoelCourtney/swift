use derive_more::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign, Sum};
use serde::{Deserialize, Serialize};

#[repr(transparent)]
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Serialize,
    Deserialize,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Neg,
    Add,
    AddAssign,
    Sum,
    Sub,
    SubAssign,
    Mul,
    MulAssign,
    Div,
    DivAssign,
)]
pub struct Duration(pub i64);

impl Duration {
    pub fn zero() -> Duration {
        Duration(0)
    }
}

pub trait Durative {
    fn duration(&self) -> Duration;
}
