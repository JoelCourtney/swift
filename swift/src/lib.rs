//! # Swift Engine
//!
//! A discrete event simulation engine with optimal incremental simulation
//! and parallelism.

use crate::exec::{BumpedFuture, ExecEnvironment, SendBump};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Debug;
pub use swift_macros::{activity, model};
use tokio::sync::RwLockReadGuard;

pub mod exec;
pub mod history;
pub mod operation;
pub mod reexports;

pub use hifitime::Duration;
pub use hifitime::Epoch;

pub trait Resource<'h>: Sized {
    const PIECEWISE_CONSTANT: bool;
    type Read: 'h + Copy + Send + Sync + Serialize;
    type Write: 'static
        + From<Self::Read>
        + Clone
        + Default
        + Debug
        + Serialize
        + DeserializeOwned
        + Send
        + Sync;

    type History: HasHistory<'h, Self> + Default;
}

pub trait Plan<'o>: Sync {
    type Model: Model<'o>;

    fn insert(&mut self, time: Epoch, activity: impl Activity<'o, Self::Model> + 'o) -> ActivityId;
    fn remove(&self, id: ActivityId);
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct ActivityId(u32);
impl ActivityId {
    pub fn new(id: u32) -> ActivityId {
        ActivityId(id)
    }
}

pub trait HasResource<'o, R: Resource<'o>>: Plan<'o> {
    fn find_child(&self, time: Epoch) -> &'o dyn Writer<'o, R, Self::Model>;
    fn insert_operation(&mut self, time: Epoch, op: &'o dyn Writer<'o, R, Self::Model>);
}

pub trait Model<'o>: Sync {
    type Plan: Plan<'o, Model = Self>;
    type InitialConditions;
    type Histories: 'o + Sync + Default;

    fn new_plan(
        time: Epoch,
        initial_conditions: Self::InitialConditions,
        bump: &'o SendBump,
    ) -> Self::Plan;
}

pub struct Timeline<'o, R: Resource<'o>, M: Model<'o>>(BTreeMap<Epoch, &'o (dyn Writer<'o, R, M>)>)
where
    M::Plan: HasResource<'o, R>;

impl<'o, R: Resource<'o>, M: Model<'o>> Timeline<'o, R, M>
where
    M::Plan: HasResource<'o, R>,
{
    pub fn init(time: Epoch, initial_condition: &'o (dyn Writer<'o, R, M>)) -> Timeline<'o, R, M> {
        Timeline(BTreeMap::from([(time, initial_condition)]))
    }

    pub fn last(&self) -> &'o (dyn Writer<'o, R, M>) {
        *self.0.last_key_value().unwrap().1
    }

    pub fn last_before(&self, time: Epoch) -> (Epoch, &'o (dyn Writer<'o, R, M>)) {
        let t = self.0.range(..time).next_back().unwrap_or_else(|| {
            panic!("No writers found before {time}. Did you insert before the initial conditions?")
        });
        (*t.0, *t.1)
    }

    pub fn first_after(&self, time: Epoch) -> Option<(Epoch, &'o (dyn Writer<'o, R, M>))> {
        self.0.range(time..).next().map(move |t| (*t.0, *t.1))
    }

    pub fn insert(&mut self, time: Epoch, value: &'o (dyn Writer<'o, R, M>)) {
        self.0.insert(time, value);
    }
}

// Auto implemented for models that contain all the resources the activity touches
pub trait Activity<'o, M: Model<'o>>: Send + Sync {
    fn decompose(&'o self, start: Epoch, plan: &mut M::Plan, bump: &'o SendBump);
}

#[async_trait]
pub trait Operation<'o, M: Model<'o>>: Sync {
    async fn find_children(&self, time: Epoch, plan: &M::Plan);
    async fn add_parent(&self, parent: &'o dyn Operation<'o, M>);
    async fn remove_parent(&self, parent: &dyn Operation<'o, M>);
}

pub trait Writer<'o, R: Resource<'o>, M: Model<'o>>: Operation<'o, M> {
    fn read<'b>(
        &'o self,
        histories: &'o M::Histories,
        env: ExecEnvironment<'b>,
    ) -> BumpedFuture<'b, (u64, RwLockReadGuard<'o, <R as Resource<'o>>::Read>)>
    where
        'o: 'b;
}

pub trait HasHistory<'h, R: Resource<'h>> {
    fn insert(&'h self, hash: u64, value: R::Write) -> R::Read;
    fn get(&'h self, hash: u64) -> Option<R::Read>;
}
