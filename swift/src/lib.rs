//! # Swift Engine
//!
//! A discrete event spacecraft simulation engine designed for schedulers.
//!
//! Swift always does the minimal amount of computation to respond to changes in the plan, and to
//! calculate only the requested resources *at the requested times*. If you only care about a couple
//! resources in the vicinity of a small plan change, then that's all the engine simulates.
//!
//! Swift also stores a permanent history of resource states, meaning that simulation is not just
//! incremental with respect to the most recent plan state; it is incremental with respect to all recorded
//! history. If you undo five recent simulated changes and add one new activity, the engine will only
//! simulate change of adding the activity, not of adding one and deleting five.
//!
//! Swift performs all simulation with as much parallelism as is mathematically allowed by the
//! configuration of the plan. Even on linear plan structures with no available concurrency, initial (extremely informal) benchmarking
//! suggests that Swift's engine overhead is significantly lower than Aerie-Merlin's - simulating
//! millions of simple operations per second instead of thousands. Highly expensive operations may
//! amortize the overhead differences, but will not amortize the parallelism.
//!
//! ## Concepts
//!
//! The engine simulates the evolution of a set of Resources over time, operated on by a
//! set of instantaneous Operations, which themselves are grouped together into Activities.
//!
//! ### Resources & Activities
//!
//! Resources are just variables whose evolution over time is tracked and recorded. Activities
//! contain operations that mutate those resources, and place those operations at specific
//! pre-determined times throughout a plan. This is the only fundamental difference between Swift and
//! Merlin; activities declare their operations - when they happen, what resources they read/write -
//! and their total duration ahead of time, before simulation.
//!
//! ### Operations, Dependencies, & Parallelism
//!
//! Operations are the instantaneous discrete events that the engine simulates. The can read and write
//! resources, access activity arguments, and be configured by the activity (ahead of time only).
//! By forcing you to declare which resources you read and write, the engine is able to build a
//! directed acyclic graph of operation dependencies. This DAG enables most of the parallelism and minimal
//! computation I bragged about in the intro. When you make a change and request a view of a resource,
//! the simulation propagates backward through the DAG from the requested range, and evaluation
//! of branches in the graph immediately stop when they find cached values from previous runs.
//!
//! ### History & Incremental Simulation
//!
//! Swift records the history of all operations that have been simulated. Currently, this is only
//! recorded per-session, but a persistent system could be implemented in the future. This enables
//! the engine to immediately stop as soon as it encounters a state that it has been in before. Importantly,
//! it recognizes the state using only the structure of the DAG and the initial conditions, not the
//! resource state at the time the operation was previously run. It does this by inductively calculating
//! hashes for each operation: each operation hashes together its own unique ID and the hashes of its dependencies,
//! and only the initial condition operations hash the input. This allows the engine to recognize past
//! states without performing any simulation.
//!
//! Importantly, Swift stores history independent of the plan, meaning that it can be shared between
//! branched versions of the same plan, even as they are updated and simulated live, in parallel.
//! For an extremely simplified example, consider a plan working on two mostly-independent subsystems,
//! `A` and `B`. We start with an unsimulated base plan, then branch into two copies for the `A` and
//! `B` teams to work on. Say team `A` simulates their portion of the base plan first. `B`'s work is
//! only *mostly* independent, with some coupling between common resources. Most of the time, `B` doesn't
//! need `A`'s resources, but if they do, `A` has already simulated the base plan and those results can
//! be reused even though they are from a different plan. Then, when the branches are merged, a majority
//! of the final plan has already been simulated. Only the areas that coupled `A` and `B` together need
//! to be resimulated.
//!
//! This approach's main drawback is memory usage. By indiscriminately storing all sim results without
//! knowing if they will ever be reused, it can build up gigabytes of store after simulating on the
//! order of tens of millions of operations. Since the keys in the storage are meaningless hashes,
//! there is currently no good way to prune the history to reduce memory usage.
//!
//! ### Models
//!
//! For those familiar with Aerie-Merlin, you might notice that I didn't use the word "Model"
//! in the above descriptions. This is because while in Merlin, the model is a container that creates,
//! specifies, and owns its resources and activities, in Swift the model is just a selection of
//! pre-existing resources. Activities are applicable to any model that selects the resources it
//! operates on. This reinterpretation gives a couple advantages:
//! - Easier modularity for levels-of-fidelity. If two models are nearly the same, except one uses
//!   higher fidelity simulation for one subsystem, all the activities that *don't* touch that subsystem
//!   are trivially applicable to both models.
//! - Shared history between models that share resources. History is recorded by resource, not
//!   by model or plan. If the same sub-graph appears in different plans on different models, the history
//!   can still be reused.
//!
//!
//! ## Modelling quick-start
//!
//! First, you need resources to operate on. For each resource, you need a vacant type to act as a
//! label:
//!
//! ```
//! # fn main() {}
//! /// Counts the number of sols that have elapsed.
//! enum SolCounter {}
//! ```
//!
//! We need the type to act as a label because we need to differentiate between resources that are
//! represented with the same data type (which in this case will be `u32`). Next, implement the [Resource]
//! trait for the label:
//!
//! ```
//! # fn main() {}
//! # use swift::Resource;
//! # use swift::CopyHistory;
//! enum SolCounter {}
//!
//! impl<'h> Resource<'h> for SolCounter {
//!     const STATIC: bool = true;
//!     type Read = u32;
//!     type Write = u32;
//!     type History = CopyHistory<'h, SolCounter>;
//! }
//! ```
//!
//! See the [Resource] trait for more details on what these types mean and how to implement it. Say
//! our model has both the `SolCounter` resource and a `DownlinkBuffer` resource represented by a `Vec<String>`.
//! We can make an activity that logs the current sol to the buffer:
//!
//! ```
//! # fn main() {}
//! # use serde::{Serialize, Deserialize};
//! # use swift::{activity, Resource, CopyHistory, DerefHistory, Duration};
//! # enum SolCounter {}
//! # impl<'h> Resource<'h> for SolCounter {
//! #     const STATIC: bool = true;
//! #     type Read = u32;
//! #     type Write = u32;
//! #     type History = CopyHistory<'h, SolCounter>;
//! # }
//! # enum DownlinkBuffer {}
//! # impl<'h> Resource<'h> for DownlinkBuffer {
//! #     const STATIC: bool = true;
//! #     type Read = &'h [String];
//! #     type Write = Vec<String>;
//! #     type History = DerefHistory<'h, DownlinkBuffer>;
//! # }
//! #[derive(Serialize, Deserialize, Clone)]
//! struct LogCurrentSol {
//!     /// Verbosity is taken in as an activity argument.
//!     verbose: bool,
//! }
//!
//! activity! {
//!     for LogCurrentSol {
//!         // This is syntactic sugar to declare an operation.
//!         // It occurs at time `start`, reads both `SolCounter` and `DownlinkBuffer`,
//!         // and writes to `DownlinkBuffer`.
//!         @(start) sol: SolCounter, buf: DownlinkBuffer -> buf {
//!             // Activity arguments are accessible under `args`, not `self`.
//!             if args.verbose {
//!                 buf.push(format!("It is currently Sol {sol}"));
//!             } else {
//!                 buf.push(format!("Sol {sol}"));
//!             }
//!         }
//!         Duration::ZERO // Return statement indicates the activity had zero duration
//!     }
//! }
//! ```
//!
//! Lastly you need to make a model that uses these resources:
//!
//! ```
//! # fn main() {}
//! # use serde::{Serialize, Deserialize};
//! # use swift::{activity, Resource, CopyHistory, DerefHistory, Duration, model};
//! # enum SolCounter {}
//! # impl<'h> Resource<'h> for SolCounter {
//! #     const STATIC: bool = true;
//! #     type Read = u32;
//! #     type Write = u32;
//! #     type History = CopyHistory<'h, SolCounter>;
//! # }
//! # enum DownlinkBuffer {}
//! # impl<'h> Resource<'h> for DownlinkBuffer {
//! #     const STATIC: bool = true;
//! #     type Read = &'h [String];
//! #     type Write = Vec<String>;
//! #     type History = DerefHistory<'h, DownlinkBuffer>;
//! # }
//! model! {
//!     DemoModel {
//!         sol: SolCounter,
//!         log: DownlinkBuffer
//!     }
//! }
//! ```
//!
//! This implements the [Model] trait, and generates structs to store initial conditions, [Plans][Plan],
//! and histories.

use crate::exec::{ExecEnvironment, SyncBump, EXECUTOR, NUM_THREADS};
pub use history::{CopyHistory, DerefHistory};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::ops::RangeBounds;
pub use swift_macros::{activity, model};
pub mod exec;
pub mod history;
pub mod operation;
pub mod reexports;
pub mod timeline;

pub use hifitime::Duration;
pub use hifitime::Epoch as Time;
use history::HasHistory;
use timeline::HasResource;

/// Marks a type as a resource label.
///
/// This trait is not applied to the actual data and is never instantiated, because multiple resources
/// might have the same representation. (i.e. both memory usage and battery state are f32's.) To enforce
/// that the resource type is only used as a label and never instantiated, it's recommended to make
/// it an empty enum (i.e. a vacant type).
///
/// Resources are not part of a model, the model is a selection of existing resources. This allows
/// activities, which are also not part of a model, to be applied to any model that has the relevant
/// resources.
///
/// ## Reading & Writing
///
/// Resources are not represented one data type, but two, one for reading and one for writing.
/// For simple [Copy] resources these two types will be the same, and you won't have to worry about it.
/// For more complex resources they may be different but related types, like [String] and [&str][str].
/// This is for performance reasons, to avoid unnecessary cloning of heap-allocated data.
///
/// The `Read` type is the input to an operation; it is what's read from history and from other operations.
/// The `Write` type is the output of an operation, and the actual type written and stored in the history.
/// Inside an operation, the type of the actual resource variable depends on how you use it.
/// - read-only: `Read`
/// - write-only: `Write`, initialized to the default value
/// - read and write: `Write`, initialized from the converted value read from history (likely cloned).
///
/// ## Choosing a history container
///
/// Currently, there are two types of storage for history ([CopyHistory] and [DerefHistory]), and
/// which you use depends on the properties of the type.
///
/// ### Copy
///
/// For a resource called `MyResource`, if the type written
/// from operations is [Copiable][std::marker::Copy], then you should use `CopyHistory<MyResource>`.
/// This requires that the `Read` and `Write` types are equal. For example:
///
/// ```
/// # fn main() {}
/// # use swift::Resource;
/// # use swift::CopyHistory;
/// enum SolCounter {}
///
/// impl<'h> Resource<'h> for SolCounter {
///     const STATIC: bool = true;
///     type Read = u32;
///     type Write = u32;
///     type History = CopyHistory<'h, SolCounter>;
/// }
/// ```
///
/// ### Deref
///
/// If the written type has a ["stable deref"][stable_deref_trait::StableDeref], meaning that it
/// dereferences to data whose address doesn't change even if the data is moved, then you can use [DerefHistory].
/// Common examples include [String], which dereferences to `str`; [`Vec<T>`], which derefs to `[T]`,
/// and [`Box<T>`], which derefs to `T`. All three of these types use heap allocations; so if you have
/// a [String], you can have an `&str` reference to its underlying data that remains valid even if
/// the [String] is moved (using a bit of unsafe code in the [elsa crate](https://docs.rs/elsa/latest/elsa/)).
///
/// If this describes your type, you can use `DerefHistory<MyResource>`, which requires that `Read = &*Write`.
/// (i.e. `Write = Vec<T>`, `Read = &[T]`, etc). For Example:
///
/// ```
/// # fn main() {}
/// # use swift::Resource;
/// # use swift::DerefHistory;
/// enum MissionPhase {}
///
/// impl<'h> Resource<'h> for MissionPhase {
///     const STATIC: bool = true;
///
///     // Note the `'h` lifetime specifier. This means that the reference
///     // lives as long as the history container itself.
///     type Read = &'h str;
///
///     type Write = String;
///     type History = DerefHistory<'h, MissionPhase>;
/// }
/// ```
///
/// ### Non-copy, non-deref types
///
/// If your type is neither copy nor stable-deref, currently the only option is to wrap it in a `Box`
/// and use `DerefHistory`. I may implement a `CloneHistory` in the future.
pub trait Resource<'h>: 'static + Sized {
    /// Whether the resource represents a value that can vary even when not actively written to by
    /// an operation. This is used for cache invalidation,
    /// so it is very important not to give false positives.
    ///
    /// The basic question is "does it matter
    /// how long it has been since this resource was last written to?" For a boolean resource, the
    /// answer is "no", and so it is static. But for a continuously-varying linear function,
    /// the answer is "yes"; the value it represents changes over time even in between operations.
    const STATIC: bool;

    /// The type that is read from history.
    type Read: 'h + Copy + Send + Sync + Serialize;

    /// The type that is written from operations to history.
    type Write: 'static
        + From<Self::Read>
        + Clone
        + Default
        + Debug
        + Serialize
        + DeserializeOwned
        + Send
        + Sync;

    /// The type of history container to use to store instances of the `Write` type, currently
    /// either [CopyHistory] or [DerefHistory]. See [Resource] for details.
    type History: HasHistory<'h, Self> + Default;
}

/// The interface that plan objects provide.
///
/// ## Constructing
///
/// The [model] macro, when applied to a [Model] struct `MyModel`, will also generate a type called
/// `MyModelPlan`, that implements this trait. It can be created with `MyModel::new_plan`.
pub trait Plan<'o>: Sync
where
    Self: 'o,
{
    type Model: Model<'o>;

    /// Inserts a new activity into the plan, and returns its unique ID.
    fn insert(
        &mut self,
        start_time: Time,
        activity: impl Activity<'o, Self::Model> + 'o,
    ) -> ActivityId;

    /// Removes an activity from the plan, by ID.
    fn remove(&self, id: ActivityId);

    /// Returns a view into a section of a resource's timeline. After creating a plan, call
    /// `plan.view::<MyResource>(start..end, &histories)` to get a vector of times and values
    /// within the `start - end` range.
    ///
    /// Try to limit the requested range to only the times that you need.
    ///
    /// The histories struct will be autogenerated by the [model] macro.
    fn view<R: Resource<'o>>(
        &self,
        bounds: impl RangeBounds<Time>,
        histories: &'o <Self::Model as Model<'o>>::Histories,
    ) -> Vec<(Time, R::Read)>
    where
        Self: HasResource<'o, R>,
    {
        let bump = SyncBump::new();
        let nodes = self.get_operations(bounds).into_iter();
        let env = ExecEnvironment::new(&bump);
        std::thread::scope(move |scope| {
            // EXPLANATION:
            // The async executor crate provides an `executor.run(fut)` function,
            // that runs the executor until `fut` completes. Importantly, if `fut` yields,
            // the executor will keep doing other submitted tasks until `fut` wakes,
            // even if they are unrelated.

            // If `fut` is, say, awaiting an async shutdown signal, then the executor
            // will keep doing any other available tasks until the shutdown signal is received.
            // The following line creates that shutdown signal. It will be sent when
            // `_signal` goes out of scope, which will only happen after all the
            // tasks we actually care about are complete.
            let (_signal, shutdown) = async_channel::bounded::<()>(1);

            for _ in 0..NUM_THREADS {
                let shutdown = shutdown.clone();
                scope.spawn(move || futures::executor::block_on(EXECUTOR.run(shutdown.recv())));
            }

            futures::executor::block_on(futures::future::join_all(
                nodes.map(|(t, n)| async move { (t, *n.read(histories, env).await.1) }),
            ))
        })
    }
}

/// A selection of resources, with tools for creating a plan and storing history.
///
/// Autogenerated by the [model] macro.
pub trait Model<'o>: Sync {
    type Plan: Plan<'o, Model = Self>;
    type InitialConditions;
    type Histories: 'o + Sync + Default;

    /// Creates a new plan instance, given a start time, initial conditions, and an allocator.
    fn new_plan(
        time: Time,
        initial_conditions: Self::InitialConditions,
        bump: &'o SyncBump,
    ) -> Self::Plan;
}

/// An activity, which decomposes into a statically-known set of operations. Implemented
/// with the [activity] macro.
pub trait Activity<'o, M: Model<'o>>: Send + Sync {
    fn decompose(&'o self, start: Time, plan: &mut M::Plan, bump: &'o SyncBump) -> Duration;
}

/// A unique activity ID.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct ActivityId(u32);
impl ActivityId {
    pub fn new(id: u32) -> ActivityId {
        ActivityId(id)
    }
}
