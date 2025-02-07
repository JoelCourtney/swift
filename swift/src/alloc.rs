use bumpalo::Bump;
use derive_more::Deref;
use std::future::Future;
use std::pin::Pin;

pub type BumpedFuture<'b, T> = Pin<&'b mut (dyn Future<Output = T> + Send + 'b)>;

#[derive(Deref, Default)]
pub struct SendBump(Bump);
unsafe impl Send for SendBump {}
unsafe impl Sync for SendBump {}

impl SendBump {
    pub fn new() -> Self {
        Self(Bump::new())
    }
}
