use crate::activity::Placement::Grounded;
use crate::operation::Node;
use crate::timeline::Timelines;
use crate::{Model, Time};
use anyhow::Result;
use bumpalo_herd::Member;
use hifitime::Duration;
use serde::{Deserialize, Serialize};
use std::ops::Add;

/// An activity, which decomposes into a statically-known set of operations. Implemented
/// with the [impl_activity] macro.
pub trait Activity<'o, M: Model<'o>>: Send + Sync {
    fn decompose(
        &'o self,
        start: Placement,
        timelines: &Timelines<'o, M>,
        bump: &Member<'o>,
    ) -> Result<(Duration, Vec<&'o dyn Node<'o, M>>)>;
}

pub trait ActivityLabel {
    const LABEL: &'static str;
}

pub enum Placement {
    Grounded(Time),
    // Ungrounded { min: Time, max: Time },
}

impl Add<Duration> for Placement {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        match self {
            Grounded(t) => Grounded(t + rhs),
        }
    }
}

/// A unique activity ID.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub struct ActivityId(u32);
impl ActivityId {
    pub fn new(id: u32) -> ActivityId {
        ActivityId(id)
    }
}
