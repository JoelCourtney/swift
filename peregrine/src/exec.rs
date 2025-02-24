#![doc(hidden)]

use bumpalo_herd::Herd;
use std::future::Future;
use std::pin::Pin;

pub const STACK_LIMIT: u16 = 1000;

#[derive(Copy, Clone)]
pub struct ExecEnvironment<'b> {
    pub herd: &'b Herd,
    pub stack_counter: u16,
}

impl<'b> ExecEnvironment<'b> {
    pub fn new(herd: &'b Herd) -> Self {
        ExecEnvironment {
            herd,
            stack_counter: 0,
        }
    }

    pub fn increment(self) -> Self {
        ExecEnvironment {
            herd: self.herd,
            stack_counter: self.stack_counter + 1,
        }
    }

    pub fn reset(&self) -> Self {
        ExecEnvironment {
            herd: self.herd,
            stack_counter: 0,
        }
    }
}

pub type BumpedFuture<'b, T> = Pin<&'b mut (dyn Future<Output = T> + Send + 'b)>;
