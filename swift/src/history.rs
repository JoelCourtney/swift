use std::hash::{BuildHasher, Hasher};
use std::ops::Deref;

use dashmap::DashMap;
use elsa::sync::FrozenMap;
use stable_deref_trait::StableDeref;

pub type SwiftDefaultHashBuilder = foldhash::fast::FixedState;

pub trait History<'a, R> where Self: 'a {
    type HistoryEntry: Copy + 'a;

    fn insert(&'a self, hash: u64, value: R) -> Self::HistoryEntry;
    fn get(&'a self, hash: u64) -> Option<Self::HistoryEntry>;
}

pub struct CopyHistory<R: Copy>(DashMap<u64, R, PassThroughHashBuilder>);

impl<'a, R: Copy + 'a> History<'a, R> for CopyHistory<R> {
    type HistoryEntry = R;

    fn insert(&'a self, hash: u64, value: R) -> R {
        self.0.insert(hash, value).unwrap()
    }

    fn get(&'a self, hash: u64) -> Option<R> {
        self.0.get(&hash).map(|r| *r)
    }
}

pub struct IndirectHistory<R: StableDeref>(FrozenMap<u64, R>);

impl<'a, R: StableDeref + 'a> History<'a, R> for IndirectHistory<R> where Self: 'a {
    type HistoryEntry = &'a <R as Deref>::Target;

    fn insert(&'a self, hash: u64, value: R) -> Self::HistoryEntry {
        self.0.insert(hash, value)
    }

    fn get(&'a self, hash: u64) -> Option<Self::HistoryEntry> {
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
