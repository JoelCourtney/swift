use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use std::hash::{BuildHasher, Hasher};

use crate::resource::Resource;

pub type SwiftDefaultHashBuilder = foldhash::fast::FixedState;

pub type History<R> = DashMap<u64, R, PassThroughHashBuilder>;

pub trait AsyncMap<R: Resource> {
    fn insert_async(&self, hash: u64, value: R) -> Option<R>;
    fn get_async(&self, hash: u64) -> Option<Ref<u64, R>>;
}

impl<R: Resource> AsyncMap<R> for History<R> {
    fn insert_async(&self, hash: u64, value: R) -> Option<R> {
        tokio::task::block_in_place(|| self.insert(hash, value))
    }

    fn get_async(&self, hash: u64) -> Option<Ref<u64, R>> {
        tokio::task::block_in_place(|| self.get(&hash))
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
