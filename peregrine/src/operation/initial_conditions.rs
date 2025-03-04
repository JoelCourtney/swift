use crate::Model;
use crate::exec::ExecEnvironment;
use crate::history::PeregrineDefaultHashBuilder;
use crate::operation::{Continuation, Node, Upstream};
use crate::resource::{ErasedResource, Resource};
use crate::timeline::Timelines;
use anyhow::anyhow;
use hifitime::Duration;
use parking_lot::{Mutex, RwLock, RwLockWriteGuard};
use rayon::Scope;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::hash::BuildHasher;

#[macro_export]
macro_rules! initial_conditions {
    ($($res:ident: $val:expr),*$(,)?) => {
        $crate::operation::initial_conditions::InitialConditions::new()
            $(.insert::<$res>($val))*
    };
}

pub struct InitialConditions(HashMap<u64, Box<dyn ErasedResource<'static>>>);

impl Default for InitialConditions {
    fn default() -> Self {
        Self::new()
    }
}

impl InitialConditions {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    pub fn insert<R: Resource<'static> + 'static>(mut self, value: R::Write) -> Self {
        let value: WriteValue<'static, R> = WriteValue(value);
        self.0.insert(value.id(), Box::new(value));
        self
    }
    pub fn take<R: Resource<'static> + 'static>(&mut self) -> Option<R::Write> {
        unsafe {
            self.0
                .remove(&R::ID)
                .map(|v| v.downcast_owned::<WriteValue<'static, R>>().0)
        }
    }
}

struct WriteValue<'h, R: Resource<'h>>(R::Write);

impl<'h, R: Resource<'h>> ErasedResource<'h> for WriteValue<'h, R> {
    fn id(&self) -> u64 {
        R::ID
    }
}

pub struct InitialConditionOp<'o, R: Resource<'o>, M: Model<'o>> {
    value: R::Write,
    result: RwLock<Option<(u64, R::Read)>>,
    downstreams: Mutex<SmallVec<Continuation<'o, R, M>, 2>>,
    _time: Duration,
}

impl<'o, R: Resource<'o>, M: Model<'o>> InitialConditionOp<'o, R, M> {
    pub fn new(time: Duration, value: R::Write) -> Self {
        Self {
            value,
            result: RwLock::new(None),
            downstreams: Mutex::default(),
            _time: time,
        }
    }
}

impl<'o, R: Resource<'o>, M: Model<'o>> Node<'o, M> for InitialConditionOp<'o, R, M> {
    fn insert_self(
        &'o self,
        _timelines: &mut Timelines<'o, M>,
        _disruptive: bool,
    ) -> anyhow::Result<()> {
        unreachable!()
    }

    fn remove_self(&self, _timelines: &mut Timelines<'o, M>) -> anyhow::Result<()> {
        Err(anyhow!("Cannot remove initial conditions."))
    }
}

impl<'o, R: Resource<'o> + 'o, M: Model<'o>> Upstream<'o, R, M> for InitialConditionOp<'o, R, M> {
    fn request<'s>(
        &'o self,
        continuation: Continuation<'o, R, M>,
        scope: &Scope<'s>,
        timelines: &'s Timelines<'o, M>,
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

        if let Some(c) = continuation.copy_node() {
            self.downstreams.lock().push(c);
        }

        continuation.run(Ok(read.unwrap()), scope, timelines, env.increment());
    }

    fn notify_downstreams(&self, time_of_change: Duration) {
        self.downstreams
            .lock()
            .retain(|downstream| match downstream {
                Continuation::Node(n) => n.clear_upstream(Some(time_of_change)),
                Continuation::MarkedNode(_, n) => n.clear_upstream(Some(time_of_change)),
                _ => unreachable!(),
            });
    }
}
