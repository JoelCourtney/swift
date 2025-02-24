#![doc(hidden)]

use bumpalo::Bump;
use derive_more::Deref;
use std::future::Future;
use std::pin::Pin;

pub const STACK_LIMIT: u16 = 1000;

#[derive(Copy, Clone)]
pub struct ExecEnvironment<'b> {
    pub bump: &'b SyncBump,
    pub stack_counter: u16,
}

impl<'b> ExecEnvironment<'b> {
    pub fn new(b: &'b SyncBump) -> Self {
        ExecEnvironment {
            bump: b,
            stack_counter: 0,
        }
    }

    pub fn increment(self) -> Self {
        ExecEnvironment {
            bump: self.bump,
            stack_counter: self.stack_counter + 1,
        }
    }
}

pub type BumpedFuture<'b, T> = Pin<&'b mut (dyn Future<Output = T> + Send + 'b)>;

#[derive(Deref, Default)]
pub struct SyncBump(Bump);
unsafe impl Sync for SyncBump {}

impl SyncBump {
    pub fn new() -> Self {
        Self(Bump::new())
    }
}
