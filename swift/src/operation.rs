#![doc(hidden)]

use std::collections::BTreeMap;
use std::hash::BuildHasher;
use std::pin::Pin;

use crate::alloc::{BumpedFuture, SendBump};
use crate::history::SwiftDefaultHashBuilder;
use crate::operation::ShouldSpawn::{No, Yes};
use crate::{HasResource, History, Operation, Resource, Time, Writer};
use async_trait::async_trait;
use tokio::sync::{RwLock, RwLockReadGuard};

#[derive(Copy, Clone, Debug, Default, Eq, PartialOrd, PartialEq)]
pub enum ShouldSpawn {
    #[default]
    Yes,
    No(u16),
}

impl ShouldSpawn {
    pub const STACK_LIMIT: u16 = 1000;

    pub fn increment(self) -> Self {
        match self {
            Yes => No(0),
            No(n) if n < Self::STACK_LIMIT => No(n + 1),
            No(Self::STACK_LIMIT) => Yes,
            _ => unreachable!(),
        }
    }
}

pub struct InitialConditionOp<R: Resource> {
    lock: RwLock<R::Read>,
}

#[async_trait]
impl<R: Resource, T: HasResource<R>> Operation<T> for InitialConditionOp<R> {
    async fn find_children(&self, _time: Time, _timelines: &T) {}

    async fn add_parent(&self, _parent: &dyn Operation<T>) {
        todo!()
    }
}

impl<R: Resource, T: HasResource<R>> Writer<R, T> for InitialConditionOp<R> {
    fn read<'a>(&'a self, _history: &dyn History<R>, _should_spawn: ShouldSpawn, b: &'a SendBump) -> BumpedFuture<'a, (u64, RwLockReadGuard<'a, R::Read>)> {
        unsafe {
            Pin::new_unchecked(b.alloc(async move {
                (
                    SwiftDefaultHashBuilder::default().hash_one(
                        bincode::serde::encode_to_vec(
                            &*(self.lock.try_read().unwrap()),
                            bincode::config::standard(),
                        )
                        .unwrap(),
                    ),
                    self.lock.read().await
                )
            }))
        }
    }
}

pub struct OperationTimeline<'a, R: Resource, T: HasResource<R>>(
    BTreeMap<Time, &'a dyn Writer<R, T>>
);

impl<'a, R: Resource, T: HasResource<R>> OperationTimeline<'a, R, T> {
    pub fn init(time: Time, initial_condition: &dyn Writer<R, T>) -> OperationTimeline<R, T> {
        OperationTimeline(BTreeMap::from([(
            time,
            initial_condition
        )]))
    }

    pub fn last(&self) -> &dyn Writer<R, T> {
        *self.0.last_key_value().unwrap().1
    }

    pub fn last_before(&self, time: Time) -> (Time, &dyn Writer<R, T>) {
        let t = self.0.range(..time).next_back().unwrap();
        (*t.0, *t.1)
    }

    pub fn first_after(&self, time: Time) -> Option<(Time, &dyn Writer<R, T>)> {
        self.0.range(time..).next().map(|t| (*t.0, *t.1))
    }

    pub fn insert(&'a mut self, time: Time, value: &'a dyn Writer<R, T>) {
        self.0.insert(time, value);
    }
}
