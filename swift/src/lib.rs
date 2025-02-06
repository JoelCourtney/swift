//! # Swift Engine
//!
//! A discrete event simulation engine with optimal incremental simulation
//! and parallelism.

use crate::alloc::{BumpedFuture, SendBump};
use crate::operation::ShouldSpawn;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
pub use swift_macros::Durative;
use tokio::sync::RwLockReadGuard;

pub mod alloc;
pub mod history;
pub mod macros;
pub mod operation;
pub mod reexports;

pub type Duration = time::Duration;
pub type Time = time::PrimitiveDateTime;

pub trait Resource: Sized {
    const PIECEWISE_CONSTANT: bool;

    type Read: From<Self::Write> + Copy + Send + Sync + Serialize;
    type Write: From<Self::Read> + Clone + Default + Debug + Serialize + for<'a> Deserialize<'a> + Send;

    type History: for<'a> History<'a, Self>;
}

pub struct Plan<M: Model> {
    _activities: Vec<(Time, Box<dyn Activity<M>>)>,
    _operations: M::Timelines
}

pub trait Model {
    type Timelines: Timelines;
}

pub trait Timelines {}

pub trait HasResource<R: Resource>: Timelines {
    fn find_child(&self, time: Time) -> &dyn Writer<R, Self>;
}

// Auto implemented for models that contain all the resources the activity touches
pub trait Activity<M: Model> {
    fn run(&self, start: Time) -> Vec<(Time, Box<dyn Operation<M::Timelines>>)>;
}

#[async_trait]
pub trait Operation<T: Timelines>: Sync {
    async fn find_children(&self, time: Time, timelines: &T);
    async fn add_parent(&self, parent: &dyn Operation<T>);
}

pub trait Writer<R: Resource, T: HasResource<R>>: Operation<T> {
    fn read<'a>(&'a self, history: &dyn History<R>, should_spawn: ShouldSpawn, b: &'a SendBump) -> BumpedFuture<'a, (u64, RwLockReadGuard<'a, R::Read>)>;
}

pub trait History<'a, R: Resource> where Self: 'a {
    fn insert(&'a self, hash: u64, value: R::Write) -> R::Read;
    fn get(&'a self, hash: u64) -> Option<R::Read>;
}
