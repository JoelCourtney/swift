//! # Swift Engine
//!
//! A discrete event simulation engine with optimal incremental simulation
//! and parallelism.
//!
//! (WIP) See [Session], [model] and [impl_activity] for details.

use serde::{Deserialize, Serialize};

use crate::operation::GroundedOperationBundle;

pub mod duration;
pub mod history;
pub mod macros;
pub mod operation;
pub mod reexports;
pub mod resource;

pub use duration::{Duration, Durative};
pub use resource::Resource;
pub use swift_macros::Durative;

/// An interactive session with cached simulation history and lazy evaluation.
pub struct Session<M: Model> {
    pub history: M::History,
    pub op_timelines: M::OperationTimelines,
}

impl<M: Model> Default for Session<M> {
    fn default() -> Self {
        Session {
            history: M::History::default(),
            op_timelines: M::OperationTimelines::default(),
        }
    }
}

/// The trait that all models implement.
///
/// Do not implement manually. Use the [model] macro.
pub trait Model: Sized {
    type History: Default;
    type OperationTimelines: Default;
    type State: Default;
}

impl<M: Model> Session<M> {
    pub async fn add(&mut self, start: Duration, activity: impl Activity<Model = M>) {
        for trigger in activity.decompose(start) {
            trigger.1.unpack(trigger.0, &mut self.op_timelines).await
        }
    }
}

/// The trait that all activities implement.
///
/// Do not implement manually. Use the [impl_activity] macro.
pub trait Activity: Durative + Serialize + for<'a> Deserialize<'a> {
    type Model: Model;

    fn decompose(self, start: Duration) -> Vec<GroundedOperationBundle<Self::Model>>;
}
