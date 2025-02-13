#![doc(hidden)]

use derive_more::{
    Add, AddAssign, Display, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign, Sum,
};
use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Sub, SubAssign};

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
    Hash,
)]
pub struct Duration(i64);

impl Duration {
    pub const fn zero() -> Duration {
        Duration(0)
    }

    pub const fn microseconds(n: i64) -> Self {
        Self(n)
    }

    pub const fn milliseconds(n: i64) -> Self {
        Self(n * 1000)
    }

    pub const fn seconds(n: i64) -> Self {
        Self(n * 1000000)
    }

    pub const fn minutes(n: i64) -> Self {
        Self(n * 1000000 * 60)
    }

    pub const fn hours(n: i64) -> Self {
        Self(n * 1000000 * 60 * 60)
    }

    pub const fn days(n: i64) -> Self {
        Self(n * 1000000 * 60 * 60 * 24)
    }
}

#[repr(transparent)]
#[derive(
    Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize, Hash, Debug, Display,
)]
pub struct Time(i64);

impl Time {
    pub fn zero_todo() -> Self {
        Self(0)
    }
}

impl Add<Duration> for Time {
    type Output = Time;

    fn add(self, rhs: Duration) -> Self::Output {
        Time(self.0 + rhs.0)
    }
}

impl AddAssign<Duration> for Time {
    fn add_assign(&mut self, rhs: Duration) {
        self.0 += rhs.0
    }
}

impl Sub<Duration> for Time {
    type Output = Time;

    fn sub(self, rhs: Duration) -> Self::Output {
        Time(self.0 - rhs.0)
    }
}

impl SubAssign<Duration> for Time {
    fn sub_assign(&mut self, rhs: Duration) {
        self.0 -= rhs.0
    }
}
