use crate::Model;
use crate::exec::ExecEnvironment;
use crate::history::PeregrineDefaultHashBuilder;
use crate::operation::{Continuation, Node, NodeVec, Upstream};
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

    fn insert_self(&'o self, _timelines: &mut M::Timelines) -> anyhow::Result<()> {
        unreachable!()
    }

    fn remove_self(&self, _timelines: &mut M::Timelines) -> anyhow::Result<()> {
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
