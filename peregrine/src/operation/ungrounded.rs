use crate::exec::ExecEnvironment;
use crate::operation::{
    Continuation, Downstream, InternalResult, Node, ObservedErrorOutput, Upstream,
};
use crate::resource::Resource;
use crate::{Model, resource};
use hifitime::Duration;
use parking_lot::Mutex;
use rayon::Scope;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::fmt::Debug;
use std::marker::PhantomData;

pub trait UngroundedUpstream<'o, R: Resource<'o>, M: Model<'o> + 'o>:
    AsRef<dyn Upstream<'o, R, M>> + Upstream<'o, peregrine_grounding, M>
{
}

use crate as peregrine;
use crate::timeline::Timelines;

resource!(pub peregrine_grounding: Duration);

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "peregrine::reexports::serde")]
pub enum Marked<'o, R: Resource<'o>> {
    Unit,
    Phantom(PhantomData<&'o R>),
}

impl<'o, R: 'o + Resource<'o>> Resource<'o> for Marked<'o, R> {
    const LABEL: &'static str = R::LABEL;
    const STATIC: bool = R::STATIC;
    const ID: u64 = peregrine_macros::random_u64!();
    type Read = MarkedValue<R::Read>;
    type Write = MarkedValue<R::Write>;
    type History = ();
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MarkedValue<T> {
    pub(crate) marker: usize,
    pub(crate) value: T,
}

impl<T: Copy + Clone + Debug> Copy for MarkedValue<T> {}
impl<T: Clone + Debug> Clone for MarkedValue<T> {
    fn clone(&self) -> Self {
        MarkedValue {
            marker: self.marker,
            value: self.value.clone(),
        }
    }
}

pub struct UngroundedUpstreamResolver<'o, R: Resource<'o>, M: Model<'o>> {
    time: Duration,
    downstream: Option<&'o dyn Downstream<'o, R, M>>,
    grounded_upstream: Option<(Duration, &'o dyn Upstream<'o, R, M>)>,
    ungrounded_upstreams: SmallVec<&'o dyn UngroundedUpstream<'o, R, M>, 1>,
    grounding_responses: Mutex<SmallVec<InternalResult<MarkedValue<Duration>>, 1>>,
    continuation: Mutex<Option<Continuation<'o, R, M>>>,

    #[allow(clippy::type_complexity)]
    cached_decision: Mutex<Option<InternalResult<(Duration, &'o dyn Upstream<'o, R, M>)>>>,
}

impl<'o, R: Resource<'o>, M: Model<'o>> UngroundedUpstreamResolver<'o, R, M> {
    pub(crate) fn new(
        time: Duration,
        grounded: Option<(Duration, &'o dyn Upstream<'o, R, M>)>,
        ungrounded: SmallVec<&'o dyn UngroundedUpstream<'o, R, M>, 1>,
    ) -> Self {
        Self {
            time,
            downstream: None,
            grounded_upstream: grounded,
            ungrounded_upstreams: ungrounded,
            grounding_responses: Mutex::new(SmallVec::new()),
            continuation: Mutex::new(None),
            cached_decision: Mutex::new(None),
        }
    }
}

impl<'o, R: Resource<'o>, M: Model<'o>> Node<'o, M> for UngroundedUpstreamResolver<'o, R, M> {
    fn insert_self(
        &'o self,
        _timelines: &mut Timelines<'o, M>,
        _disruptive: bool,
    ) -> anyhow::Result<()> {
        unreachable!()
    }

    fn remove_self(&self, _timelines: &mut Timelines<'o, M>) -> anyhow::Result<()> {
        unreachable!()
    }

    fn clear_cache(&self) {
        *self.cached_decision.lock() = None;
        if let Some(d) = self.downstream {
            d.clear_cache();
        }
    }

    fn notify_downstreams(&self, time_of_change: Duration) {
        if let Some(d) = self.downstream {
            d.clear_upstream(Some(time_of_change));
        }
    }
}

impl<'o, R: Resource<'o>, M: Model<'o>> Upstream<'o, R, M>
    for UngroundedUpstreamResolver<'o, R, M>
{
    fn request<'s>(
        &'o self,
        continuation: Continuation<'o, R, M>,
        scope: &Scope<'s>,
        timelines: &'s Timelines<'o, M>,
        env: ExecEnvironment<'s, 'o>,
    ) where
        'o: 's,
    {
        let decision = self.cached_decision.lock();
        if let Some(r) = *decision {
            match r {
                Ok((_, u)) => u.request(continuation, scope, timelines, env.increment()),
                Err(_) => {
                    continuation.run(Err(ObservedErrorOutput), scope, timelines, env.increment())
                }
            }
            return;
        }
        drop(decision);

        let mut continuation_lock = self.continuation.lock();
        debug_assert!(continuation_lock.is_none());
        *continuation_lock = Some(continuation);
        drop(continuation_lock);

        for (i, ungrounded) in self.ungrounded_upstreams[1..].iter().enumerate() {
            scope.spawn(move |s| {
                ungrounded.request(
                    Continuation::<peregrine_grounding, M>::MarkedNode(i, self),
                    s,
                    timelines,
                    env.reset(),
                )
            });
        }
    }
}

impl<'o, R: Resource<'o>, M: Model<'o>> Downstream<'o, Marked<'o, peregrine_grounding>, M>
    for UngroundedUpstreamResolver<'o, R, M>
{
    fn respond<'s>(
        &'o self,
        value: InternalResult<(u64, MarkedValue<Duration>)>,
        scope: &Scope<'s>,
        timelines: &'s Timelines<'o, M>,
        env: ExecEnvironment<'s, 'o>,
    ) where
        'o: 's,
    {
        let mut responses_lock = self.grounding_responses.lock();
        responses_lock.push(value.map(|ok| ok.1));

        if responses_lock.len() == self.ungrounded_upstreams.len() {
            let folded_result = responses_lock
                .drain(..)
                .collect::<anyhow::Result<SmallVec<_, 1>, _>>();
            let mut decision = self.cached_decision.lock();
            let continuation = self.continuation.lock().take().unwrap();
            match folded_result {
                Err(_) => {
                    *decision = Some(Err(ObservedErrorOutput));
                    continuation.run(Err(ObservedErrorOutput), scope, timelines, env.increment());
                }
                Ok(vec) => {
                    let earliest_ungrounded = vec
                        .iter()
                        .filter(|gr| gr.value < self.time)
                        .max_by_key(|gr| gr.value);

                    match (earliest_ungrounded, self.grounded_upstream) {
                        (Some(ug), Some(gr)) => {
                            if gr.0 > ug.value {
                                *decision = Some(Ok(gr));
                            } else {
                                *decision = Some(Ok((
                                    ug.value,
                                    self.ungrounded_upstreams[ug.marker].as_ref(),
                                )));
                            }
                        }
                        (Some(ug), None) => {
                            *decision = Some(Ok((
                                ug.value,
                                self.ungrounded_upstreams[ug.marker].as_ref(),
                            )))
                        }
                        (None, Some(gr)) => *decision = Some(Ok(gr)),
                        _ => unreachable!(),
                    }

                    decision.unwrap().unwrap().1.request(
                        continuation,
                        scope,
                        timelines,
                        env.increment(),
                    );
                }
            }
        }
    }

    fn clear_upstream(&self, _time_of_change: Option<Duration>) -> bool {
        if let Some(d) = self.downstream {
            d.clear_upstream(_time_of_change)
        } else {
            false
        }
    }
}
