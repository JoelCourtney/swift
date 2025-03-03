use crate::exec::ExecEnvironment;
use crate::operation::{
    Continuation, Downstream, InternalResult, Node, ObservedErrorOutput, Upstream,
};
use crate::resource::Resource;
use crate::timeline::HasTimeline;
use crate::{Model, resource};
use hifitime::Duration;
use parking_lot::Mutex;
use rayon::Scope;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

pub trait UngroundedUpstream<'o, R: Resource<'o>, M: Model<'o> + 'o>: Upstream<'o, R, M> {
    fn ground_request<'s>(
        &'o self,
        marker: usize,
        continuation: Continuation<'o, peregrine_grounding, M>,
        scope: &Scope<'s>,
        timelines: &M::Timelines,
        env: ExecEnvironment<'s, 'o>,
    );
    fn upcast(&'o self) -> &'o dyn Upstream<'o, R, M>;
}

use crate as peregrine;
resource!(pub peregrine_grounding: GroundingResponse);

#[derive(Copy, Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct GroundingResponse {
    marker: usize,
    when: Duration,
}

pub struct UngroundedUpstreamResolver<'o, R: Resource<'o>, M: Model<'o>> {
    time: Duration,
    downstream: &'o dyn Downstream<'o, R, M>,
    ungrounded_upstreams: SmallVec<&'o dyn UngroundedUpstream<'o, R, M>, 1>,
    grounding_responses: Mutex<SmallVec<InternalResult<GroundingResponse>, 1>>,
    grounded_upstream: Option<(Duration, &'o dyn Upstream<'o, R, M>)>,
    continuation: Mutex<Option<Continuation<'o, R, M>>>,

    #[allow(clippy::type_complexity)]
    cached_decision: Mutex<Option<InternalResult<(Duration, &'o dyn Upstream<'o, R, M>)>>>,
}

impl<'o, R: Resource<'o>, M: Model<'o>> Node<'o, M> for UngroundedUpstreamResolver<'o, R, M>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    fn insert_self(&'o self, _timelines: &mut M::Timelines) -> anyhow::Result<()> {
        unreachable!()
    }

    fn remove_self(&self, _timelines: &mut M::Timelines) -> anyhow::Result<()> {
        unreachable!()
    }

    fn clear_cache(&self) {
        *self.cached_decision.lock() = None;
        self.downstream.clear_cache();
    }

    fn notify_downstreams(&self, time_of_change: Duration) {
        self.downstream.clear_upstream(Some(time_of_change));
    }
}

impl<'o, R: Resource<'o>, M: Model<'o>> Upstream<'o, R, M> for UngroundedUpstreamResolver<'o, R, M>
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
                ungrounded.ground_request(i, Continuation::Node(self), s, timelines, env.reset())
            });
        }
    }
}

impl<'o, R: Resource<'o>, M: Model<'o>> Downstream<'o, peregrine_grounding, M>
    for UngroundedUpstreamResolver<'o, R, M>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    fn respond<'s>(
        &'o self,
        value: InternalResult<(u64, GroundingResponse)>,
        scope: &Scope<'s>,
        timelines: &'s M::Timelines,
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
                        .filter(|gr| gr.when < self.time)
                        .max_by_key(|gr| gr.when);

                    match (earliest_ungrounded, self.grounded_upstream) {
                        (Some(ug), Some(gr)) => {
                            if gr.0 > ug.when {
                                *decision = Some(Ok(gr));
                            } else {
                                *decision = Some(Ok((
                                    ug.when,
                                    self.ungrounded_upstreams[ug.marker].upcast(),
                                )));
                            }
                        }
                        (Some(ug), None) => {
                            *decision =
                                Some(Ok((ug.when, self.ungrounded_upstreams[ug.marker].upcast())))
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
        unreachable!()
    }
}
