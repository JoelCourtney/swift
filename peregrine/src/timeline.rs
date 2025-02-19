#![doc(hidden)]

use crate::operation::Writer;
use crate::{Model, Resource};
use hifitime::Epoch as Time;
use std::collections::BTreeMap;
use std::ops::RangeBounds;

pub trait HasTimeline<'o, R: Resource<'o>, M: Model<'o>> {
    fn find_child(&self, time: Time) -> &'o dyn Writer<'o, R, M>;
    fn insert_operation(&mut self, time: Time, op: &'o dyn Writer<'o, R, M>);

    fn get_operations(
        &self,
        bounds: impl RangeBounds<Time>,
    ) -> Vec<(Time, &'o dyn Writer<'o, R, M>)>;
}

pub struct Timeline<'o, R: Resource<'o>, M: Model<'o>>(BTreeMap<Time, &'o (dyn Writer<'o, R, M>)>)
where
    M::Timelines: HasTimeline<'o, R, M>;

impl<'o, R: Resource<'o>, M: Model<'o>> Timeline<'o, R, M>
where
    M::Timelines: HasTimeline<'o, R, M>,
{
    pub fn init(time: Time, initial_condition: &'o (dyn Writer<'o, R, M>)) -> Timeline<'o, R, M> {
        Timeline(BTreeMap::from([(time, initial_condition)]))
    }

    pub fn last(&self) -> (Time, &'o (dyn Writer<'o, R, M>)) {
        let tup = self.0.last_key_value().unwrap();
        (*tup.0, *tup.1)
    }

    pub fn last_before(&self, time: Time) -> (Time, &'o (dyn Writer<'o, R, M>)) {
        let tup = self.0.range(..time).next_back().unwrap_or_else(|| {
            panic!("No writers found before {time}. Did you insert before the initial conditions?")
        });
        (*tup.0, *tup.1)
    }

    pub fn first_after(&self, time: Time) -> Option<(Time, &'o (dyn Writer<'o, R, M>))> {
        self.0.range(time..).next().map(move |t| (*t.0, *t.1))
    }

    pub fn insert(&mut self, time: Time, value: &'o (dyn Writer<'o, R, M>)) {
        self.0.insert(time, value);
    }

    pub fn range<'a>(
        &'a self,
        range: impl RangeBounds<Time>,
    ) -> impl Iterator<Item = (Time, &'o (dyn Writer<'o, R, M>))> + 'a {
        self.0.range(range).map(|(t, w)| (*t, *w))
    }
}
