#![doc(hidden)]

pub mod initial_conditions;
pub mod ungrounded;

use crate::Model;
use crate::exec::ExecEnvironment;
use crate::operation::ungrounded::{Marked, MarkedValue};
use crate::resource::Resource;
use crate::timeline::Timelines;
use anyhow::Result;
use derive_more::with_trait::Error as DeriveError;
use hifitime::Duration;
use rayon::Scope;
use smallvec::SmallVec;
use std::fmt::{Debug, Display, Formatter};

pub type InternalResult<T> = Result<T, ObservedErrorOutput>;

pub trait Node<'o, M: Model<'o> + 'o>: Sync {
    fn insert_self(&'o self, timelines: &mut Timelines<'o, M>, disruptive: bool) -> Result<()>;
    fn remove_self(&self, timelines: &mut Timelines<'o, M>) -> Result<()>;
}

pub trait Downstream<'o, R: Resource<'o>, M: Model<'o> + 'o>: Node<'o, M> {
    fn respond<'s>(
        &'o self,
        value: InternalResult<(u64, R::Read)>,
        scope: &Scope<'s>,
        timelines: &'s Timelines<'o, M>,
        env: ExecEnvironment<'s, 'o>,
    ) where
        'o: 's;

    fn clear_cache(&self);
    fn clear_upstream(&self, time_of_change: Option<Duration>) -> bool;
}

pub trait Upstream<'o, R: Resource<'o>, M: Model<'o> + 'o>: Node<'o, M> {
    fn request<'s>(
        &'o self,
        continuation: Continuation<'o, R, M>,
        scope: &Scope<'s>,
        timelines: &'s Timelines<'o, M>,
        env: ExecEnvironment<'s, 'o>,
    ) where
        'o: 's;

    fn notify_downstreams(&self, time_of_change: Duration);
}

pub enum Continuation<'o, R: Resource<'o>, M: Model<'o> + 'o> {
    Node(&'o dyn Downstream<'o, R, M>),
    MarkedNode(usize, &'o dyn Downstream<'o, Marked<'o, R>, M>),
    Root(oneshot::Sender<InternalResult<R::Read>>),
}

impl<'o, R: Resource<'o>, M: Model<'o> + 'o> Continuation<'o, R, M> {
    pub fn run<'s>(
        self,
        value: InternalResult<(u64, R::Read)>,
        scope: &Scope<'s>,
        timelines: &'s Timelines<'o, M>,
        env: ExecEnvironment<'s, 'o>,
    ) where
        'o: 's,
    {
        match self {
            Continuation::Node(n) => n.respond(value, scope, timelines, env),
            Continuation::MarkedNode(marker, n) => n.respond(
                value.map(|(hash, when)| {
                    (
                        hash,
                        MarkedValue {
                            marker,
                            value: when,
                        },
                    )
                }),
                scope,
                timelines,
                env,
            ),
            Continuation::Root(s) => s.send(value.map(|r| r.1)).unwrap(),
        }
    }

    pub fn copy_node(&self) -> Option<Self> {
        match &self {
            Continuation::Node(n) => Some(Continuation::Node(*n)),
            Continuation::MarkedNode(m, n) => Some(Continuation::MarkedNode(*m, *n)),
            _ => None,
        }
    }
}

pub struct RecordedQueue<N, O> {
    pub new: SmallVec<N, 1>,
    pub old: SmallVec<O, 1>,
}

impl<N, O> Default for RecordedQueue<N, O> {
    fn default() -> Self {
        Self::new()
    }
}

impl<N, O> RecordedQueue<N, O> {
    pub fn new() -> Self {
        Self {
            new: SmallVec::new(),
            old: SmallVec::new(),
        }
    }
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
pub struct ObservedErrorOutput;

impl Display for ObservedErrorOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "encountered a stale error from a previous run")
    }
}

pub type NodeVec<'o, M> = SmallVec<&'o dyn Node<'o, M>, 2>;
pub type UpstreamVec<'o, R, M> = SmallVec<&'o dyn Upstream<'o, R, M>, 2>;

#[derive(Eq, PartialEq, Debug, Copy, Clone, Default)]
pub enum OperationState {
    #[default]
    Dormant,
    Waiting,
    Done,
}
