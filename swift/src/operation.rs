#![doc(hidden)]

use crate::exec::{BumpedFuture, ExecEnvironment, SendBump};
use crate::history::{HasHistory, SwiftDefaultHashBuilder};
use crate::timeline::HasResource;
use crate::{Activity, ActivityId, Model, Plan, Resource, Time};
use async_trait::async_trait;
use std::hash::BuildHasher;
use std::ops::RangeBounds;
use std::pin::Pin;
use tokio::sync::{RwLock, RwLockReadGuard};

#[async_trait]
pub trait Operation<'o, M: Model<'o>>: Sync {
    async fn find_children(&self, time: Time, plan: &M::Plan);
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

pub struct InitialConditionOpInner<'o, R: Resource<'o>, M: Model<'o>>
where
    M::Plan: HasResource<'o, R>,
{
    value: <R as Resource<'o>>::Write,
    result: Option<(u64, <R as Resource<'o>>::Read)>,
    parents: Vec<&'o dyn Operation<'o, M>>,
}

pub struct InitialConditionOp<'o, R: Resource<'o>, M: Model<'o>>
where
    M::Plan: HasResource<'o, R>,
{
    lock: RwLock<InitialConditionOpInner<'o, R, M>>,
}

impl<'o, R: Resource<'o>, M: Model<'o>> InitialConditionOp<'o, R, M>
where
    M::Plan: HasResource<'o, R>,
{
    pub fn new(value: <R as Resource<'o>>::Write) -> Self {
        Self {
            lock: RwLock::new(InitialConditionOpInner {
                value,
                result: None,
                parents: vec![],
            }),
        }
    }
}

#[async_trait]
impl<'o, R: Resource<'o>, M: Model<'o>> Operation<'o, M> for InitialConditionOp<'o, R, M>
where
    M::Plan: HasResource<'o, R>,
{
    async fn find_children(&self, _time: Time, _plan: &M::Plan) {}

    async fn add_parent(&self, parent: &'o dyn Operation<'o, M>) {
        let mut write = self.lock.write().await;
        write.parents.push(parent);
    }

    async fn remove_parent(&self, parent: &dyn Operation<'o, M>) {
        let mut write = self.lock.write().await;
        write.parents.retain(|p| !std::ptr::eq(*p, parent));
    }
}

impl<'o, R: Resource<'o> + 'o, M: Model<'o>> Writer<'o, R, M> for InitialConditionOp<'o, R, M>
where
    M::Histories: HasHistory<'o, R>,
    M::Plan: HasResource<'o, R>,
{
    fn read<'b>(
        &'o self,
        histories: &'o M::Histories,
        env: ExecEnvironment<'b>,
    ) -> BumpedFuture<'b, (u64, RwLockReadGuard<'o, <R as Resource<'o>>::Read>)>
    where
        'o: 'b,
    {
        unsafe {
            Pin::new_unchecked(env.bump.alloc(async move {
                let read_guard = if let Ok(mut write_guard) = self.lock.try_write() {
                    if write_guard.result.is_none() {
                        let hash = SwiftDefaultHashBuilder::default().hash_one(
                            bincode::serde::encode_to_vec(
                                &write_guard.value,
                                bincode::config::standard(),
                            )
                            .unwrap(),
                        );
                        if let Some(r) = histories.get(hash) {
                            write_guard.result = Some((hash, r));
                        } else {
                            write_guard.result =
                                Some((hash, histories.insert(hash, write_guard.value.clone())));
                        }
                    }
                    write_guard.downgrade()
                } else {
                    self.lock.read().await
                };
                let hash = read_guard.result.unwrap().0;
                (
                    hash,
                    RwLockReadGuard::map(read_guard, |o| &o.result.as_ref().unwrap().1),
                )
            }))
        }
    }
}

pub enum AllModel {}

impl<'o> Model<'o> for AllModel {
    type Plan = AllPlan;
    type InitialConditions = ();
    type Histories = ();

    fn new_plan(
        _time: Time,
        _initial_conditions: Self::InitialConditions,
        _bump: &'o SendBump,
    ) -> AllPlan {
        unimplemented!()
    }
}

pub enum AllPlan {}

impl<'o> Plan<'o> for AllPlan {
    type Model = AllModel;

    fn insert(
        &mut self,
        _time: Time,
        _activity: impl Activity<'o, Self::Model> + 'o,
    ) -> ActivityId {
        unimplemented!()
    }

    fn remove(&self, _id: ActivityId) {
        unimplemented!()
    }
}

impl<R: Resource<'static>> HasResource<'static, R> for AllPlan {
    fn find_child(&self, _time: Time) -> &'static dyn Writer<'static, R, Self::Model> {
        unimplemented!()
    }

    fn insert_operation(&mut self, _time: Time, _op: &'static dyn Writer<'static, R, Self::Model>) {
        unimplemented!()
    }

    fn get_operations(
        &self,
        _bounds: impl RangeBounds<Time>,
    ) -> Vec<(Time, &'static dyn Writer<'static, R, Self::Model>)> {
        todo!()
    }
}
