use std::future::Future;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;

use crate::duration::{Duration, Durative};
use crate::operation::OperationBundle;
use crate::resource::Resource;

pub mod duration;
mod example;
pub mod resource;
pub mod history;
pub mod operation;
pub mod macros;
pub mod reexports;

pub use swift_macros::Durative;

pub struct Session<M: Model> {
    pub history: M::History,
    pub op_timelines: M::OperationTimelines
}

impl<M: Model> Default for Session<M> {
    fn default() -> Self {
        Session {
            history: M::History::default(),
            op_timelines: M::OperationTimelines::default()
        }
    }
}

pub trait Model: Sized {
    type History: Default;
    type OperationTimelines: Default;
}

impl<M: Model> Session<M> {
    pub async fn add(&mut self, start: Duration, activity: impl Activity<Model=M>) {
        for trigger in activity.decompose(start) {
            trigger.1.unpack(trigger.0, &mut self.op_timelines).await
        }
    }
}

pub trait Activity: Durative + Serialize + for<'a> Deserialize<'a> {
    type Model: Model;

    fn decompose(self, start: Duration) -> Vec<(Duration, Box<dyn OperationBundle<Self::Model>>)>;
}
