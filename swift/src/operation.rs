#![doc(hidden)]

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

pub struct InitialConditionOp<'h, R: Resource<'h>> {
    lock: RwLock<<R as Resource<'h>>::Read>,
}

#[async_trait]
impl<'h, R: Resource<'h>, T: HasResource<'h, R>> Operation<T> for InitialConditionOp<'h, R> {
    async fn find_children(&self, _time: Time, _timelines: &T) {}

    async fn add_parent(&self, _parent: &dyn Operation<T>) {
        todo!()
    }
}

impl<'h, R: Resource<'h>, T: HasResource<'h, R>> Writer<'h, R, T> for InitialConditionOp<'h, R> {
    fn read<'b: 'h>(&'b self, _history: &dyn History<R>, _should_spawn: ShouldSpawn, b: &'b SendBump) -> BumpedFuture<'b, (u64, RwLockReadGuard<'b, <R as Resource<'h>>::Read>)> {
        unsafe {
            Pin::new_unchecked(b.alloc(async move {
                (
                    SwiftDefaultHashBuilder::default().hash_one(
                        bincode::serde::encode_to_vec(
                            *(self.lock.try_read().unwrap()),
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




