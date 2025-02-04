#![doc(hidden)]

use crate::duration::Duration;
use serde::{Deserialize, Serialize};

pub trait Resource:
    'static + Default + Send + Sync + Serialize + for<'a> Deserialize<'a> + Clone
{
    const PIECEWISE_CONSTANT: bool;
}

pub trait ResourceTypeTag {
    type ResourceType: Resource;
}

macro_rules! impl_resource {
    ($constant:literal $($ty:ident)*) => {
        $(
            impl Resource for $ty {
                const PIECEWISE_CONSTANT: bool = $constant;
            }
        )*
    };
}

impl_resource! {
    true // all these are piecewise constant

    u8 i8
    u16 i16
    u32 i32
    u64 i64
    u128 i128
    usize isize

    f32 f64

    bool

    String char

    Duration

}

impl<R: Resource> Resource for Box<R> {
    const PIECEWISE_CONSTANT: bool = R::PIECEWISE_CONSTANT;
}

impl<R: Resource> Resource for Vec<R> {
    const PIECEWISE_CONSTANT: bool = R::PIECEWISE_CONSTANT;
}
