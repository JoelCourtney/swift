use crate::operation::Node;
use crate::{Grounding, Model};
use anyhow::Result;
use bumpalo_herd::Member;
use hifitime::Duration;
use serde::{Deserialize, Serialize};

/// An activity, which decomposes into a statically-known set of operations. Implemented
/// with the [impl_activity] macro.
pub trait Activity<'o, M: Model<'o>>: Send + Sync {
    fn decompose(
        &'o self,
        start: Grounding<'o, M>,
        bump: Member<'o>,
    ) -> Result<(Duration, Vec<&'o dyn Node<'o, M>>)>;
}

pub trait ActivityLabel {
    const LABEL: &'static str;
}

/// A unique activity ID.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub struct ActivityId(u32);
impl ActivityId {
    pub fn new(id: u32) -> ActivityId {
        ActivityId(id)
    }
}
