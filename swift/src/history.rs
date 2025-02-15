#![doc(hidden)]

use std::hash::{BuildHasher, Hasher};
use std::ops::Deref;

use crate::Resource;
use dashmap::DashMap;
use elsa::sync::FrozenMap;
use stable_deref_trait::StableDeref;

pub type SwiftDefaultHashBuilder = foldhash::fast::FixedState;

pub trait HasHistory<'h, R: Resource<'h>> {
    fn insert(&'h self, hash: u64, value: R::Write) -> R::Read;
    fn get(&'h self, hash: u64) -> Option<R::Read>;
}

#[derive(Debug)]
pub struct CopyHistory<'h, R: Resource<'h>>(
    DashMap<u64, <R as Resource<'h>>::Write, PassThroughHashBuilder>,
)
where
    <R as Resource<'h>>::Write: Copy;

impl<'h, R: Resource<'h>> Default for CopyHistory<'h, R>
where
    <R as Resource<'h>>::Write: Copy,
{
    fn default() -> Self {
        CopyHistory(DashMap::default())
    }
}

impl<'h, V: Copy + 'h, R: for<'b> Resource<'b, Read = V, Write = V> + 'h> HasHistory<'h, R>
    for CopyHistory<'h, R>
{
    fn insert(&self, hash: u64, value: <R as Resource<'_>>::Write) -> <R as Resource<'_>>::Read {
        self.0.insert(hash, value);
        value
    }

    fn get(&self, hash: u64) -> Option<<R as Resource<'_>>::Read> {
        self.0.get(&hash).map(|r| *r)
    }
}

#[derive(Debug)]
pub struct DerefHistory<'h, R: Resource<'h>>(FrozenMap<u64, <R as Resource<'h>>::Write>)
where
    <R as Resource<'h>>::Write: StableDeref;

impl<'h, R: Resource<'h>> Default for DerefHistory<'h, R>
where
    <R as Resource<'h>>::Write: StableDeref,
{
    fn default() -> Self {
        DerefHistory(FrozenMap::default())
    }
}

impl<'h, V: StableDeref + 'h, R: Resource<'h, Write = V, Read = &'h <V as Deref>::Target>>
    HasHistory<'h, R> for DerefHistory<'h, R>
where
    Self: 'h,
{
    fn insert(&'h self, hash: u64, value: <R as Resource<'h>>::Write) -> <R as Resource<'h>>::Read {
        self.0.insert(hash, value)
    }

    fn get(&'h self, hash: u64) -> Option<<R as Resource<'h>>::Read> {
        self.0.get(&hash)
    }
}

// i suspect the compiler will be able to turn this into a no-op
pub struct PassThroughHasher(u64);

impl Hasher for PassThroughHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, _bytes: &[u8]) {
        unreachable!()
    }
    fn write_u8(&mut self, _i: u8) {
        unreachable!()
    }
    fn write_u16(&mut self, _i: u16) {
        unreachable!()
    }
    fn write_u32(&mut self, _i: u32) {
        unreachable!()
    }

    fn write_u64(&mut self, i: u64) {
        self.0 = i;
    }

    fn write_usize(&mut self, _i: usize) {
        unreachable!()
    }
}

#[derive(Copy, Clone, Default)]
pub struct PassThroughHashBuilder;

impl BuildHasher for PassThroughHashBuilder {
    type Hasher = PassThroughHasher;

    fn build_hasher(&self) -> PassThroughHasher {
        PassThroughHasher(0)
    }
}
