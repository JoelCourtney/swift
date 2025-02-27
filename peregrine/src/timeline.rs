#![doc(hidden)]

use crate::Model;
use crate::operation::ungrounded::UngroundedUpstream;
use crate::operation::{Node, NodeVec, Upstream};
use crate::resource::Resource;
use derive_more::Deref;
use hifitime::TimeScale::TAI;
use hifitime::{Duration, Epoch as Time};
use smallvec::SmallVec;
use std::collections::btree_map::Range;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound::{Excluded, Unbounded};
use std::ops::RangeBounds;

pub trait HasTimeline<'o, R: Resource<'o>, M: Model<'o>> {
    fn find_child(&self, time: Duration) -> Option<&'o dyn Upstream<'o, R, M>>;

    fn insert_grounded(
        &mut self,
        time: Duration,
        op: &'o dyn Upstream<'o, R, M>,
    ) -> Option<&'o dyn Upstream<'o, R, M>>;
    fn remove_grounded(&mut self, time: Duration) -> Option<&'o dyn Node<'o, M>>;

    fn insert_ungrounded(
        &mut self,
        min: Duration,
        max: Duration,
        op: &'o dyn UngroundedUpstream<'o, R, M>,
    ) -> NodeVec<'o, M>;
    fn remove_ungrounded(&mut self, min: Duration) -> Option<&'o dyn Node<'o, M>>;

    fn get_operations(
        &self,
        bounds: impl RangeBounds<Duration>,
    ) -> Vec<(Duration, &'o dyn Upstream<'o, R, M>)>;
}

// All Epochs/Times are converted to TAI durations because the Ord implementation
// on Epoch does a timescale conversion every time, which is very inefficient.

// TAI (international atomic time) is chosen as the base representation
// because hifitime does all epoch conversions through TAI, so it is the most
// efficient format to convert to.
pub fn epoch_to_duration(time: Time) -> Duration {
    time.to_tai_duration()
}
pub fn duration_to_epoch(duration: Duration) -> Time {
    Time {
        duration,
        time_scale: TAI,
    }
}

pub struct Timeline<'o, R: Resource<'o>, M: Model<'o>>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    grounded: BTreeMap<Duration, &'o dyn Upstream<'o, R, M>>,
    ungrounded: BTreeMap<Duration, UngroundedMapEntry<'o, R, M>>,
}

#[derive(Deref)]
struct UngroundedMapEntry<'o, R: Resource<'o>, M: Model<'o>> {
    starts_here: &'o dyn UngroundedUpstream<'o, R, M>,
    #[deref]
    others_present: BTreeMap<Duration, &'o dyn UngroundedUpstream<'o, R, M>>,
}

impl<'o, R: Resource<'o>, M: Model<'o>> UngroundedMapEntry<'o, R, M> {
    fn new(starts_here: &'o dyn UngroundedUpstream<'o, R, M>, ends: Duration) -> Self {
        UngroundedMapEntry {
            starts_here,
            others_present: BTreeMap::from([(ends, starts_here)]),
        }
    }
}

impl<'o, R: Resource<'o>, M: Model<'o>> Timeline<'o, R, M>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    pub fn init(
        time: Duration,
        initial_condition: &'o dyn Upstream<'o, R, M>,
    ) -> Timeline<'o, R, M> {
        Timeline {
            grounded: BTreeMap::from([(time, initial_condition)]),
            ungrounded: BTreeMap::new(),
        }
    }

    pub fn last(&self) -> Option<(Duration, &'o dyn Upstream<'o, R, M>)> {
        self.grounded.last_key_value().map(|(t, w)| (*t, *w))
    }

    pub fn last_before(&self, time: Duration) -> Option<(Duration, &'o dyn Upstream<'o, R, M>)> {
        self.grounded
            .range(..time)
            .next_back()
            .map(|(t, w)| (*t, *w))
    }

    #[cfg(not(feature = "nightly"))]
    pub fn insert_grounded(
        &mut self,
        time: Duration,
        value: &'o (dyn Upstream<'o, R, M>),
    ) -> Option<&'o (dyn Upstream<'o, R, M>)> {
        self.grounded.insert(time, value);
        self.last_before(time).map(|(_, w)| w)
    }

    #[cfg(feature = "nightly")]
    pub fn insert_grounded(
        &mut self,
        time: Duration,
        value: &'o dyn Upstream<'o, R, M>,
    ) -> Option<&'o dyn Upstream<'o, R, M>> {
        let mut cursor_mut = self.grounded.upper_bound_mut(Unbounded);
        if let Some((t, _)) = cursor_mut.peek_prev() {
            if *t < time {
                cursor_mut.insert_after(time, value).unwrap();
                return Some(*cursor_mut.as_cursor().peek_prev().unwrap().1);
            }
        }
        self.grounded.insert(time, value);
        self.last_before(time).map(|(_, w)| w)
    }

    pub fn remove_grounded(&mut self, time: Duration) -> Option<&'o dyn Upstream<'o, R, M>> {
        self.grounded.remove(&time)
    }

    pub fn insert_ungrounded(
        &mut self,
        min: Duration,
        max: Duration,
        value: &'o dyn UngroundedUpstream<'o, R, M>,
    ) -> NodeVec<'o, M> {
        let mut entry = UngroundedMapEntry::new(value, max);
        entry.extend(
            self.ungrounded
                .range(..min)
                .next_back()
                .map(|(_, entry)| entry.range((Excluded(min), Unbounded)))
                .unwrap_or(Range::default()),
        );

        self.ungrounded.insert(min, entry);
        todo!()
    }

    pub fn range<'a>(
        &'a self,
        range: impl RangeBounds<Duration>,
    ) -> impl Iterator<Item = (Duration, &'o dyn Upstream<'o, R, M>)> + 'a {
        self.grounded.range(range).map(|(t, w)| (*t, *w))
    }
}
