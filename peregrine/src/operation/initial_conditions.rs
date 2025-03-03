use crate::Model;
use crate::exec::ExecEnvironment;
use crate::history::PeregrineDefaultHashBuilder;
use crate::operation::{Continuation, DownstreamVec, Node, Upstream};
use crate::resource::Resource;
use crate::timeline::HasTimeline;
use anyhow::anyhow;
use hifitime::Duration;
use parking_lot::{Mutex, RwLock, RwLockWriteGuard};
use rayon::Scope;
use std::hash::BuildHasher;

pub struct InitialConditionOp<'o, R: Resource<'o>, M: Model<'o>>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    value: R::Write,
    result: RwLock<Option<(u64, R::Read)>>,
    downstreams: Mutex<DownstreamVec<'o, R, M>>,
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
            downstreams: Mutex::new(DownstreamVec::new()),
            _time: time,
        }
    }
}

impl<'o, R: Resource<'o>, M: Model<'o>> Node<'o, M> for InitialConditionOp<'o, R, M>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    fn insert_self(&'o self, _timelines: &mut M::Timelines) -> anyhow::Result<()> {
        unreachable!()
    }

    fn remove_self(&self, _timelines: &mut M::Timelines) -> anyhow::Result<()> {
        Err(anyhow!("Cannot remove initial conditions."))
    }

    fn clear_cache(&self) {
        unreachable!()
    }

    fn notify_downstreams(&self, time_of_change: Duration) {
        for downstream in self.downstreams.lock().drain(..) {
            downstream.clear_upstream(Some(time_of_change));
        }
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
        timelines: &'s M::Timelines,
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

        if let Some(d) = continuation.get_downstream() {
            self.downstreams.lock().push(d);
        }

        continuation.run(Ok(read.unwrap()), scope, timelines, env.increment());
    }
}
