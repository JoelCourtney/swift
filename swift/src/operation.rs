#![doc(hidden)]

use std::collections::BTreeMap;
use std::hash::BuildHasher;
use std::pin::Pin;
use std::sync::{Arc, Weak};

use crate::alloc::{BumpedFuture, SendBump};
use crate::duration::Duration;
use crate::history::SwiftDefaultHashBuilder;
use crate::operation::ShouldSpawn::{No, Yes};
use crate::resource::ResourceTypeTag;
use crate::Model;
use async_trait::async_trait;
use tokio::sync::{RwLock, RwLockReadGuard};

pub trait Operation<M: Model, TAG: ResourceTypeTag>: Send + Sync {
    fn run<'a>(
        &'a self,
        should_spawn: ShouldSpawn,
        b: &'a SendBump,
    ) -> BumpedFuture<'a, (u64, RwLockReadGuard<'a, TAG::ResourceType>)>;

    fn find_children<'a>(
        &'a self,
        time: Duration,
        timelines: &'a M::OperationTimelines,
        b: &'a SendBump,
    ) -> BumpedFuture<'a, ()>;
}

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

#[async_trait]
pub trait OperationBundle<M: Model> {
    async fn unpack(
        &self,
        time: Duration,
        timelines: &mut M::OperationTimelines,
        history: Arc<M::History>,
    );
}

pub type GroundedOperationBundle<M> = (Duration, Box<dyn OperationBundle<M>>);

pub struct OperationNode<M: Model, TAG: ResourceTypeTag> {
    op: Arc<dyn Operation<M, TAG>>,

    _parent_notifiers: Vec<Box<dyn FnOnce() + Send + Sync>>,
}

impl<M: Model, TAG: ResourceTypeTag> OperationNode<M, TAG> {
    pub fn new(
        op: Arc<dyn Operation<M, TAG>>,
        parent_notifiers: Vec<Box<dyn FnOnce() + Send + Sync>>,
    ) -> OperationNode<M, TAG> {
        OperationNode {
            op,
            _parent_notifiers: parent_notifiers,
        }
    }

    pub async fn run<'a>(&'a self, b: &'a SendBump) -> RwLockReadGuard<'a, TAG::ResourceType> {
        self.op.run(No(0), b).await.1
    }

    pub fn get_op(&self) -> Arc<dyn Operation<M, TAG>> {
        self.op.clone()
    }

    pub fn get_op_weak(&self) -> Weak<dyn Operation<M, TAG>> {
        Arc::downgrade(&self.op)
    }
}

impl<M: Model, TAG: ResourceTypeTag> Operation<M, TAG> for RwLock<TAG::ResourceType> {
    fn run<'a>(
        &'a self,
        _should_spawn: ShouldSpawn,
        b: &'a SendBump,
    ) -> BumpedFuture<'a, (u64, RwLockReadGuard<'a, TAG::ResourceType>)> {
        unsafe {
            Pin::new_unchecked(b.alloc(async move {
                (
                    SwiftDefaultHashBuilder::default().hash_one(
                        bincode::serde::encode_to_vec(
                            &*(self.try_read().unwrap()),
                            bincode::config::standard(),
                        )
                        .unwrap(),
                    ),
                    self.read().await,
                )
            }))
        }
    }

    fn find_children<'a>(
        &'a self,
        _time: Duration,
        _timelines: &'a M::OperationTimelines,
        b: &'a SendBump,
    ) -> BumpedFuture<'a, ()> {
        unsafe { Pin::new_unchecked(b.alloc(async move {})) }
    }
}

pub struct OperationTimeline<M: Model, TAG: ResourceTypeTag>(
    BTreeMap<Duration, OperationNode<M, TAG>>,
);

impl<M: Model, TAG: ResourceTypeTag> OperationTimeline<M, TAG> {
    pub fn init(value: TAG::ResourceType) -> OperationTimeline<M, TAG> {
        OperationTimeline(BTreeMap::from([(
            Duration::zero(),
            OperationNode::new(Arc::new(RwLock::new(value)), vec![]),
        )]))
    }

    pub fn last(&self) -> &OperationNode<M, TAG> {
        self.0.last_key_value().unwrap().1
    }

    pub fn last_before(&self, time: Duration) -> (&Duration, &OperationNode<M, TAG>) {
        self.0.range(..time).next_back().unwrap()
    }

    pub fn first_after(&self, time: Duration) -> Option<(&Duration, &OperationNode<M, TAG>)> {
        self.0.range(time..).next()
    }

    pub fn insert(&mut self, time: Duration, value: OperationNode<M, TAG>) {
        self.0.insert(time, value);
    }
}
