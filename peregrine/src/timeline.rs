#![doc(hidden)]

use crate::Model;
use crate::operation::Writer;
use crate::resource::Resource;
use hifitime::TimeScale::TAI;
use hifitime::{Duration, Epoch as Time};
use std::collections::BTreeMap;
use std::ops::RangeBounds;

pub trait HasTimeline<'o, R: Resource<'o>, M: Model<'o>> {
    fn find_child(&self, time: Duration) -> Option<&'o dyn Writer<'o, R, M>>;
    fn insert_operation(
        &mut self,
        time: Duration,
        op: &'o dyn Writer<'o, R, M>,
    ) -> Option<&'o dyn Writer<'o, R, M>>;
    fn remove_operation(&mut self, time: Duration) -> Option<&'o dyn Writer<'o, R, M>>;

    fn get_operations(
        &self,
        bounds: impl RangeBounds<Duration>,
    ) -> Vec<(Duration, &'o dyn Writer<'o, R, M>)>;
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

pub struct Timeline<'o, R: Resource<'o>, M: Model<'o>>(
    BTreeMap<Duration, &'o (dyn Writer<'o, R, M>)>,
)
where
    M::Timelines: HasTimeline<'o, R, M>;

impl<'o, R: Resource<'o>, M: Model<'o>> Timeline<'o, R, M>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    pub fn init(time: Duration, initial_condition: &'o dyn Writer<'o, R, M>) -> Timeline<'o, R, M> {
        Timeline(BTreeMap::from([(time, initial_condition)]))
    }

    pub fn last(&self) -> Option<(Duration, &'o dyn Writer<'o, R, M>)> {
        self.0.last_key_value().map(|(t, w)| (*t, *w))
    }

    pub fn last_before(&self, time: Duration) -> Option<(Duration, &'o dyn Writer<'o, R, M>)> {
        self.0.range(..time).next_back().map(|(t, w)| (*t, *w))
    }

    pub fn first_after(&self, time: Duration) -> Option<(Duration, &'o dyn Writer<'o, R, M>)> {
        self.0.range(time..).next().map(move |(t, w)| (*t, *w))
    }

    #[cfg(not(feature = "nightly"))]
    pub fn insert(
        &mut self,
        time: Duration,
        value: &'o (dyn Writer<'o, R, M>),
    ) -> Option<&'o (dyn Writer<'o, R, M>)> {
        self.0.insert(time, value);
        self.last_before(time).map(|(_, w)| w)
    }

    #[cfg(feature = "nightly")]
    pub fn insert(
        &mut self,
        time: Duration,
        value: &'o dyn Writer<'o, R, M>,
    ) -> Option<&'o dyn Writer<'o, R, M>> {
        let mut cursor_mut = self.0.upper_bound_mut(std::ops::Bound::Unbounded);
        if let Some((t, _)) = cursor_mut.peek_prev() {
            if *t < time {
                cursor_mut.insert_before(time, value).unwrap();
                return Some(*cursor_mut.as_cursor().peek_prev().unwrap().1);
            }
        }
        self.0.insert(time, value);
        self.last_before(time).map(|(_, w)| w)
    }

    pub fn remove(&mut self, time: Duration) -> Option<&'o (dyn Writer<'o, R, M>)> {
        self.0.remove(&time)
    }

    pub fn range<'a>(
        &'a self,
        range: impl RangeBounds<Duration>,
    ) -> impl Iterator<Item = (Duration, &'o (dyn Writer<'o, R, M>))> + 'a {
        self.0.range(range).map(|(t, w)| (*t, *w))
    }
}
