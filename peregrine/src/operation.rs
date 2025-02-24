#![doc(hidden)]

use crate::exec::{BumpedFuture, ExecEnvironment};
use crate::history::{History, PeregrineDefaultHashBuilder};
use crate::resource::Resource;
use crate::timeline::HasTimeline;
use crate::{Model, Time};
use anyhow::{Result, anyhow};
use derive_more::with_trait::Error as DeriveError;
use hifitime::Duration;
use parking_lot::Mutex;
use smallvec::SmallVec;
use std::fmt::{Debug, Display, Formatter};
use std::hash::BuildHasher;
use std::pin::Pin;
use tokio::sync::{RwLock, RwLockReadGuard};

pub trait Operation<'o, M: Model<'o> + 'o>: Sync {
    fn find_children(&'o self, time_of_change: Duration, timelines: &M::Timelines);
    fn add_parent(&self, parent: &'o dyn Operation<'o, M>);
    fn remove_parent(&self, parent: &dyn Operation<'o, M>);

    fn insert_self(&'o self, timelines: &mut M::Timelines) -> Result<()>;
    fn remove_self(&self, timelines: &mut M::Timelines) -> Result<()>;

    fn parents(&self) -> ParentsVec<'o, M>;
    fn clear_cache(&self) -> bool;

    fn notify_parents(&self, time_of_change: Duration, timelines: &M::Timelines) {
        for parent in self.parents() {
            parent.find_children(time_of_change, timelines);
        }
    }
}

pub trait Writer<'o, R: Resource<'o>, M: Model<'o> + 'o>: Operation<'o, M> {
    fn read<'b>(
        &'o self,
        histories: &'o History,
        env: ExecEnvironment<'b>,
    ) -> BumpedFuture<'b, Result<(u64, RwLockReadGuard<'o, <R as Resource<'o>>::Read>)>>
    where
        'o: 'b;
}

/// An internal marker error to signify that a node attempted to read
/// from an upstream node that had already computed an error.
///
/// This is to avoid duplicating the same error many times across all
/// branches of the graph. Instead, the true error is only returned once,
/// by the original task that computed it,
/// and all subsequent reads return this struct, which is filtered out
/// by `plan.view`.
#[derive(Copy, Clone, Debug, Default, DeriveError)]
pub struct ObservedErrorOutput(pub Time, pub &'static str);

impl Display for ObservedErrorOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "encountered a stale error from a previous run, in activity {} at {}",
            self.1, self.0
        )
    }
}

pub type ParentsVec<'o, M> = SmallVec<&'o dyn Operation<'o, M>, 2>;

pub struct InitialConditionOpInner<'o, R: Resource<'o>> {
    value: <R as Resource<'o>>::Write,
    result: Option<(u64, <R as Resource<'o>>::Read)>,
}

pub struct InitialConditionOp<'o, R: Resource<'o>, M: Model<'o>>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    lock: RwLock<InitialConditionOpInner<'o, R>>,
    parents: Mutex<ParentsVec<'o, M>>,
    _time: Duration,
}

impl<'o, R: Resource<'o>, M: Model<'o>> InitialConditionOp<'o, R, M>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    pub fn new(time: Duration, value: <R as Resource<'o>>::Write) -> Self {
        Self {
            lock: RwLock::new(InitialConditionOpInner {
                value,
                result: None,
            }),
            parents: Mutex::new(SmallVec::new()),
            _time: time,
        }
    }
}

impl<'o, R: Resource<'o>, M: Model<'o>> Operation<'o, M> for InitialConditionOp<'o, R, M>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    fn find_children(&'o self, _time_of_change: Duration, _timelines: &M::Timelines) {
        unreachable!()
    }

    fn add_parent(&self, parent: &'o dyn Operation<'o, M>) {
        self.parents.lock().push(parent);
    }

    fn remove_parent(&self, parent: &dyn Operation<'o, M>) {
        self.parents.lock().retain(|p| !std::ptr::eq(*p, parent));
    }

    fn insert_self(&'o self, _timelines: &mut M::Timelines) -> Result<()> {
        unreachable!()
    }

    fn remove_self(&self, _timelines: &mut M::Timelines) -> Result<()> {
        Err(anyhow!("Cannot remove initial conditions."))
    }

    fn parents(&self) -> ParentsVec<'o, M> {
        self.parents.lock().clone()
    }

    fn clear_cache(&self) -> bool {
        unreachable!()
    }
}

impl<'o, R: Resource<'o> + 'o, M: Model<'o>> Writer<'o, R, M> for InitialConditionOp<'o, R, M>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    fn read<'b>(
        &'o self,
        histories: &'o History,
        env: ExecEnvironment<'b>,
    ) -> BumpedFuture<'b, Result<(u64, RwLockReadGuard<'o, <R as Resource<'o>>::Read>)>>
    where
        'o: 'b,
    {
        unsafe {
            Pin::new_unchecked(env.bump.alloc(async move {
                let read_guard = if let Ok(mut write_guard) = self.lock.try_write() {
                    if write_guard.result.is_none() {
                        let hash = PeregrineDefaultHashBuilder::default().hash_one(
                            bincode::serde::encode_to_vec(
                                &write_guard.value,
                                bincode::config::standard(),
                            )?,
                        );
                        if let Some(r) = histories.get::<R>(hash) {
                            write_guard.result = Some((hash, r));
                        } else {
                            write_guard.result = Some((
                                hash,
                                histories.insert::<R>(hash, write_guard.value.clone()),
                            ));
                        }
                    }
                    write_guard.downgrade()
                } else {
                    self.lock.read().await
                };
                let hash = read_guard
                    .result
                    .ok_or_else(|| anyhow!("initial condition result not written"))?
                    .0;
                Ok((
                    hash,
                    RwLockReadGuard::map(read_guard, |o| &o.result.as_ref().unwrap().1),
                ))
            }))
        }
    }
}
