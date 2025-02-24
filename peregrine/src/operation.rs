#![doc(hidden)]

use crate::Model;
use crate::exec::ExecEnvironment;
use crate::history::PeregrineDefaultHashBuilder;
use crate::resource::Resource;
use crate::timeline::HasTimeline;
use anyhow::{Result, anyhow};
use crossbeam::queue::SegQueue;
use derive_more::Deref;
use derive_more::with_trait::Error as DeriveError;
use hifitime::Duration;
use parking_lot::{Mutex, RwLock, RwLockWriteGuard};
use rayon::Scope;
use smallvec::SmallVec;
use std::cell::UnsafeCell;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::hash::BuildHasher;

pub trait Node<'o, M: Model<'o> + 'o>: Sync {
    fn find_upstreams(&'o self, time_of_change: Duration, timelines: &M::Timelines);
    fn add_downstream(&self, node: &'o dyn Node<'o, M>);
    fn remove_downstream(&self, node: &dyn Node<'o, M>);

    fn insert_self(&'o self, timelines: &mut M::Timelines) -> Result<()>;
    fn remove_self(&self, timelines: &mut M::Timelines) -> Result<()>;

    fn clear_cache(&self) -> bool;

    fn downstreams(&self) -> NodeVec<'o, M>;
    fn notify_downstreams(&self, time_of_change: Duration, timelines: &M::Timelines) {
        for node in self.downstreams() {
            node.find_upstreams(time_of_change, timelines);
        }
    }
}

pub trait Downstream<'o, R: Resource<'o>, M: Model<'o> + 'o>: Node<'o, M> {
    fn respond<'s>(
        &'o self,
        value: Result<(u64, R::Read), ObservedErrorOutput>,
        scope: &Scope<'s>,
        env: ExecEnvironment<'s, 'o>,
    ) where
        'o: 's;
}

pub trait Upstream<'o, R: Resource<'o>, M: Model<'o> + 'o>: Node<'o, M> {
    fn request<'s>(
        &'o self,
        continuation: Continuation<'o, R, M>,
        scope: &Scope<'s>,
        env: ExecEnvironment<'s, 'o>,
    ) where
        'o: 's;
}

pub enum Continuation<'o, R: Resource<'o>, M: Model<'o> + 'o> {
    Node(&'o dyn Downstream<'o, R, M>),
    Root(oneshot::Sender<Result<R::Read, ObservedErrorOutput>>),
}

impl<'o, R: Resource<'o>, M: Model<'o> + 'o> Continuation<'o, R, M> {
    pub fn run<'s>(
        self,
        value: Result<(u64, R::Read), ObservedErrorOutput>,
        scope: &Scope<'s>,
        env: ExecEnvironment<'s, 'o>,
    ) where
        'o: 's,
    {
        match self {
            Continuation::Node(n) => n.respond(value, scope, env),
            Continuation::Root(s) => s.send(value.map(|r| r.1)).unwrap(),
        }
    }
}

//
// pub struct DynamicOperationResolver<'o, V: 'o, M: Model<'o>> {
//     time: Duration,
//     downstream: &'o dyn Node<'o, M>,
//     grounders: SmallVec<(Duration, &'o dyn Operation<'o, Duration, M>, &'o dyn Operation<'o, V, M>), 1>,
//     grounded_upstream: (Duration, &'o dyn Operation<'o, V, M>),
// }
//
// impl<'o, V: 'o, M: Model<'o>> Node<'o, M> for DynamicOperationResolver<'o, V, M> {
//     fn find_upstreams(&'o self, _time_of_change: Duration, _timelines: &M::Timelines) {
//         todo!()
//     }
//
//     fn add_downstream(&self, _parent: &'o dyn Node<'o, M>) {
//         unreachable!()
//     }
//
//     fn remove_downstream(&self, _parent: &dyn Node<'o, M>) {
//         unreachable!()
//     }
//
//     fn insert_self(&'o self, _timelines: &mut M::Timelines) -> Result<()> {
//         unreachable!()
//     }
//
//     fn remove_self(&self, _timelines: &mut M::Timelines) -> Result<()> {
//         unreachable!()
//     }
//
//     fn clear_cache(&self) -> bool {
//         todo!()
//     }
//
//     fn downstreams(&self) -> DownstreamsVec<'o, M> {
//         DownstreamsVec::from([self.downstream])
//     }
// }
//
// impl<'o, R: Resource<'o>, M: Model<'o>> Operation<'o, R, M> for DynamicOperationResolver<'o, R, M>
// where
//     M::Timelines: HasTimeline<'o, R, M>,
// {
//     fn run<'b>(
//         &'o self,
//         history: &'o History,
//         env: ExecEnvironment<'b>,
//     ) -> BumpedFuture<'b, Result<(u64, RwLockReadGuard<'o, <R as Resource<'o>>::Read>)>>
//     where
//         'o: 'b,
//     {
//         assert!(!self.grounders.is_empty());
//         env.bump_future(async move {
//             let mut latest_grounder = self.grounded_child;
//
//             for (start, delay, grounder) in &self.grounders[1..] {
//                 let time = *start + *delay.wake(history, env).await?.1;
//                 if time < self.time {
//                     match latest_grounder {
//                         Some((previous_time, _)) if previous_time < time => {
//                             latest_grounder = (time, *grounder);
//                         }
//                         None => {
//                             latest_grounder = (time, *grounder);
//                         }
//                         _ => {}
//                     }
//                 }
//             }
//
//             if let Some((t, g)) = latest_grounder {
//                 if t > self.grounded_child.0 {
//                     return g.read(history, env).await;
//                 }
//             }
//
//             self.grounded_child.1.run(history, env).await
//         })
//     }
// }

/// An internal marker error to signify that a node attempted to read
/// from an upstream node that had already computed an error.
///
/// This is to avoid duplicating the same error many times across all
/// branches of the graph. Instead, the true error is only returned once,
/// by the original task that computed it,
/// and all subsequent reads return this struct, which is filtered out
/// by `plan.view`.
#[derive(Copy, Clone, Debug, Default, DeriveError)]
pub struct ObservedErrorOutput;

impl Display for ObservedErrorOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "encountered a stale error from a previous run")
    }
}

pub type NodeVec<'o, M> = SmallVec<&'o dyn Node<'o, M>, 2>;

pub struct InitialConditionOp<'o, R: Resource<'o>, M: Model<'o>>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    value: R::Write,
    result: RwLock<Option<(u64, R::Read)>>,
    downstreams: Mutex<NodeVec<'o, M>>,
    _time: Duration,
}

impl<'o, R: Resource<'o>, M: Model<'o>> InitialConditionOp<'o, R, M>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    pub fn new(time: Duration, value: R::Write) -> Self {
        Self {
            value,
            result: RwLock::new(None),
            downstreams: Mutex::new(NodeVec::new()),
            _time: time,
        }
    }
}

impl<'o, R: Resource<'o>, M: Model<'o>> Node<'o, M> for InitialConditionOp<'o, R, M>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    fn find_upstreams(&'o self, _time_of_change: Duration, _timelines: &M::Timelines) {
        unreachable!()
    }

    fn add_downstream(&self, parent: &'o dyn Node<'o, M>) {
        self.downstreams.lock().push(parent);
    }

    fn remove_downstream(&self, parent: &dyn Node<'o, M>) {
        self.downstreams
            .lock()
            .retain(|p| !std::ptr::eq(*p, parent));
    }

    fn insert_self(&'o self, _timelines: &mut M::Timelines) -> Result<()> {
        unreachable!()
    }

    fn remove_self(&self, _timelines: &mut M::Timelines) -> Result<()> {
        Err(anyhow!("Cannot remove initial conditions."))
    }

    fn clear_cache(&self) -> bool {
        unreachable!()
    }

    fn downstreams(&self) -> NodeVec<'o, M> {
        self.downstreams.lock().clone()
    }
}

impl<'o, R: Resource<'o> + 'o, M: Model<'o>> Upstream<'o, R, M> for InitialConditionOp<'o, R, M>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    fn request<'s>(
        &'o self,
        continuation: Continuation<'o, R, M>,
        scope: &Scope<'s>,
        env: ExecEnvironment<'s, 'o>,
    ) where
        'o: 's,
    {
        let read = if let Some(mut write) = self.result.try_write() {
            if write.is_none() {
                let hash = PeregrineDefaultHashBuilder::default().hash_one(
                    bincode::serde::encode_to_vec(&self.value, bincode::config::standard())
                        .expect("could not hash initial condition"),
                );
                if let Some(r) = env.history.get::<R>(hash) {
                    *write = Some((hash, r));
                } else {
                    *write = Some((hash, env.history.insert::<R>(hash, self.value.clone())));
                }
            }
            RwLockWriteGuard::downgrade(write)
        } else {
            self.result.read()
        };

        continuation.run(Ok(read.unwrap()), scope, env.increment());
    }
}

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum OperationState {
    Dormant,
    Waiting,
    Done,
}

#[derive(Deref)]
#[repr(transparent)]
pub struct UnsyncUnsafeCell<T>(UnsafeCell<T>);
unsafe impl<T> Sync for UnsyncUnsafeCell<T> {}

impl<T> UnsyncUnsafeCell<T> {
    pub fn new(value: T) -> Self {
        Self(UnsafeCell::new(value))
    }
}

#[derive(Default, Debug)]
pub struct ErrorAccumulator(SegQueue<anyhow::Error>);
impl ErrorAccumulator {
    pub fn push(&self, err: anyhow::Error) {
        if !err.is::<ObservedErrorOutput>() {
            self.0.push(err);
        }
    }

    pub fn into_vec(self) -> Vec<anyhow::Error> {
        self.0.into_iter().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Display for ErrorAccumulator {
    fn fmt(&self, _f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl Error for ErrorAccumulator {}
