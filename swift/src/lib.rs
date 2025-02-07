//! # Swift Engine
//!
//! A discrete event simulation engine with optimal incremental simulation
//! and parallelism.

use std::collections::BTreeMap;
use crate::alloc::{BumpedFuture, SendBump};
use crate::operation::ShouldSpawn;
use async_trait::async_trait;
use serde::{Serialize};
use std::fmt::Debug;
use serde::de::DeserializeOwned;
pub use swift_macros::Durative;
use tokio::sync::RwLockReadGuard;

pub mod alloc;
pub mod history;
pub mod macros;
pub mod operation;
pub mod reexports;

pub type Duration = time::Duration;
pub type Time = time::PrimitiveDateTime;

pub trait Resource<'h>: Sized {
    const PIECEWISE_CONSTANT: bool;

    type Read: 'h + Copy + Send + Sync + Serialize;
    type Write: 'h + From<Self::Read> + Clone + Default + Debug + Serialize + DeserializeOwned + Send;

    type History: History<'h, Self>;
}

pub struct Plan<M: Model> {
    _activities: Vec<(Time, Box<dyn Activity<M>>)>,
    _operations: M
}

pub trait Model {}

pub struct Timeline<'h, R: Resource<'h>, M: HasResource<'h, R>>(
    BTreeMap<Time, &'h (dyn Writer<'h, R, M>)>
);

impl<'h, R: Resource<'h>, M: HasResource<'h, R>> Timeline<'h, R, M> {
    pub fn init(time: Time, initial_condition: &'h (dyn Writer<'h, R, M>)) -> Timeline<'h, R, M> {
        Timeline(BTreeMap::from([(
            time,
            initial_condition
        )]))
    }

    pub fn last(&'h self) -> &'h (dyn Writer<'h, R, M>) {
        *self.0.last_key_value().unwrap().1
    }

    pub fn last_before(&'h self, time: Time) -> (Time, &'h (dyn Writer<'h, R, M>)) {
        let t = self.0.range(..time).next_back().unwrap();
        (*t.0, *t.1)
    }

    pub fn first_after(&'h self, time: Time) -> Option<(Time, &'h (dyn Writer<'h, R, M>))> {
        self.0.range(time..).next().map(move |t| (*t.0, *t.1))
    }

    pub fn insert(&'h mut self, time: Time, value: &'h (dyn Writer<'h, R, M>)) {
        self.0.insert(time, value);
    }
}

pub trait HasResource<'h, R: Resource<'h>>: Model {
    fn find_child(&self, time: Time) -> &'h dyn Writer<R, Self>;
}

// Auto implemented for models that contain all the resources the activity touches
pub trait Activity<M: Model> {
    fn run(&self, start: Time) -> Vec<(Time, Box<dyn Operation<M>>)>;
}

#[async_trait]
pub trait Operation<M: Model>: Sync {
    async fn find_children(&self, time: Time, timelines: &M);
    async fn add_parent(&self, parent: &dyn Operation<M>);
}

pub trait Writer<'h, R: Resource<'h>, T: HasResource<'h, R>>: Operation<T> {
    fn read<'b: 'h>(&'b self, history: &'h dyn History<R>, should_spawn: ShouldSpawn, b: &'b SendBump) -> BumpedFuture<'b, (u64, RwLockReadGuard<'b, <R as Resource<'h>>::Read>)>;
}

pub trait History<'h, R: Resource<'h>> where Self: 'h {
    fn insert(&'h self, hash: u64, value: R::Write) -> R::Read;
    fn get(&'h self, hash: u64) -> Option<R::Read>;
}
