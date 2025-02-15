#![doc(hidden)]

use crate::exec::ShouldSpawn::*;
use async_executor::StaticExecutor;
use bumpalo::Bump;
use derive_more::Deref;
use std::future::Future;
use std::pin::Pin;

pub static EXECUTOR: StaticExecutor = StaticExecutor::new();
pub const NUM_THREADS: usize = 4;
pub const STACK_LIMIT: u16 = 1000;

#[derive(Copy, Clone)]
pub struct ExecEnvironment<'b> {
    pub bump: &'b SendBump,
    pub should_spawn: ShouldSpawn,
}

impl<'b> ExecEnvironment<'b> {
    pub fn new(b: &'b SendBump) -> Self {
        ExecEnvironment {
            bump: b,
            should_spawn: No(0),
        }
    }

    pub fn increment(self) -> Self {
        ExecEnvironment {
            bump: self.bump,
            should_spawn: self.should_spawn.increment(),
        }
    }
}

pub type BumpedFuture<'b, T> = Pin<&'b mut (dyn Future<Output = T> + Send + 'b)>;

#[derive(Deref, Default)]
pub struct SendBump(Bump);
unsafe impl Sync for SendBump {}

impl SendBump {
    pub fn new() -> Self {
        Self(Bump::new())
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialOrd, PartialEq)]
pub enum ShouldSpawn {
    #[default]
    Yes,
    No(u16),
}

impl ShouldSpawn {
    pub fn increment(self) -> Self {
        match self {
            Yes => No(0),
            No(n) if n < STACK_LIMIT => No(n + 1),
            No(STACK_LIMIT) => Yes,
            _ => unreachable!(),
        }
    }
}
