use std::any::TypeId;
use std::hash::{BuildHasher, Hash, Hasher};
use std::str::FromStr;
use std::sync::{Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio::sync::RwLockReadGuard;

use crate::{Activity, Model};
use crate::history::{History, SwiftDefaultHashBuilder};
use crate::duration::{Duration, Durative};
use crate::operation::{Operation, OperationBundle, OperationNode, OperationTimeline};
use crate::resource::ResourceTypeTag;

struct ExModel;

impl Model for ExModel {
    type History = ExHistory;
    type OperationTimelines = ExOperationTimelines;
}

enum AType {}
impl ResourceTypeTag for AType {
    type ResourceType = u32;
}

enum BType {}
impl ResourceTypeTag for BType {
    type ResourceType = String;
}

#[derive(Default)]
struct ExHistory {
    a: History<u32>,
    b: History<String>,
}

struct ExOperationTimelines {
    a: OperationTimeline<ExModel, AType>,
    b: OperationTimeline<ExModel, BType>
}

impl Default for ExOperationTimelines {
    fn default() -> Self {
        ExOperationTimelines {
            a: OperationTimeline::init(0),
            b: OperationTimeline::init(String::new())
        }
    }
}

#[derive(Serialize, Deserialize)]
struct MyActivity;

impl Durative for MyActivity {
    fn duration(&self) -> Duration {
        Duration(3)
    }
}

impl Activity for MyActivity {
    type Model = ExModel;

    fn decompose(self, start: Duration) -> Vec<(Duration, Box<dyn OperationBundle<ExModel>>)> {
        vec![
            (start, Box::new(IncrementABundle)),
            (start + Duration(2), Box::new(ConvertAToBBundle)),
            (start + Duration(3), Box::new(ComplexBundle))
        ]
    }
}

#[derive(Serialize, Deserialize)]
struct JustTheIncrement;

impl Durative for JustTheIncrement {
    fn duration(&self) -> Duration {
        Duration::zero()
    }
}

impl Activity for JustTheIncrement {
    type Model = ExModel;
    fn decompose(self, start: Duration) -> Vec<(Duration, Box<dyn OperationBundle<ExModel>>)> {
        vec![
            (start, Box::new(IncrementABundle)),
        ]
    }
}

struct IncrementABundle;

#[async_trait]
impl OperationBundle<ExModel> for IncrementABundle {
    async fn unpack(&self, time: Duration, timelines: &mut <ExModel as Model>::OperationTimelines) {
        let mut owned_parents = vec![];
        let mut ref_parents = vec![];
        if let Some((a_time, a_parent)) = timelines.a.first_after(time) {
            let p = a_parent.get_op();
            ref_parents.push(Arc::downgrade(&p));
            owned_parents.push((*a_time, p));
        }

        let (a_time, a_child) = timelines.a.last_before(time);

        let a_write_node = OperationNode::new(
            Arc::new(RwLock::new(IncrementA {
                a_child: a_child.get_op(),
                output: None,
            })),
            vec![]
        );

        timelines.a.insert(time, a_write_node);


        for (t, p) in owned_parents {
            p.find_children(t, timelines).await;
        }
    }
}

struct IncrementA {
    a_child: Arc<dyn Operation<ExModel, AType>>,
    output: Option<IncrementAOutput>
}

struct IncrementAOutput {
    a: u32
}

#[async_trait]
impl Operation<ExModel, AType> for RwLock<IncrementA> {
    async fn run(&self, history: &ExHistory) -> RwLockReadGuard<u32> {
        match self.try_write() {
            Ok(mut write) if write.output.is_none() => {
                let result = write.a_child.run(history).await.clone() + 1;
                (*write).output = Some(IncrementAOutput {
                    a: result.clone()
                });
                drop(write);
                history.a.insert(<RwLock<IncrementA> as Operation<ExModel, AType>>::history_hash(self), result);
            }
            _ => {}
        }

        return RwLockReadGuard::map(self.read().await, |o| &o.output.as_ref().unwrap().a);
    }

    fn history_hash(&self) -> u64 {
        let mut state = SwiftDefaultHashBuilder::default().build_hasher();

        TypeId::of::<ConvertAToB>().hash(&mut state);

        self.try_read().unwrap().a_child.history_hash().hash(&mut state);

        state.finish()
    }

    async fn find_children(&self, time: Duration, timelines: &<ExModel as Model>::OperationTimelines) {
        let (a_time, a_child) = timelines.a.last_before(time);

        self.write().await.a_child = a_child.get_op();
    }
}

struct ConvertAToBBundle;

#[async_trait]
impl OperationBundle<ExModel> for ConvertAToBBundle {
    async fn unpack(&self, time: Duration, timelines: &mut <ExModel as Model>::OperationTimelines) {
        let (a_time, a_child) = timelines.a.last_before(time);

        let b_write_node = OperationNode::new(
            Arc::new(RwLock::new(ConvertAToB {
                a_child: a_child.get_op(),
                output: None,
            })),
            vec![]
        );

        timelines.b.insert(time, b_write_node);
    }
}

struct ConvertAToB {
    a_child: Arc<dyn Operation<ExModel, AType>>,
    output: Option<ConvertAToBOutput>
}

struct ConvertAToBOutput {
    b: String
}

#[async_trait]
impl Operation<ExModel, BType> for RwLock<ConvertAToB> {
    async fn run(&self, history: &ExHistory) -> RwLockReadGuard<String> {
        if let Ok(mut write) = self.try_write() {
            let result = write.a_child.run(history).await.to_string();
            (*write).output = Some(ConvertAToBOutput {
                b: result.clone()
            });
            drop(write);
            history.b.insert(<RwLock<ConvertAToB> as Operation<ExModel, BType>>::history_hash(self), result);
        }

        return RwLockReadGuard::map(self.read().await, |o| &o.output.as_ref().unwrap().b);
    }

    fn history_hash(&self) -> u64 {
        let mut state = SwiftDefaultHashBuilder::default().build_hasher();

        TypeId::of::<ConvertAToB>().hash(&mut state);

        self.try_read().unwrap().a_child.history_hash().hash(&mut state);

        state.finish()
    }

    async fn find_children(&self, time: Duration, timelines: &<ExModel as Model>::OperationTimelines) {
        let (a_time, a_child) = timelines.a.last_before(time);

        self.write().await.a_child = a_child.get_op();
    }
}

struct ComplexBundle;

#[async_trait]
impl OperationBundle<ExModel> for ComplexBundle {
    async fn unpack(&self, time: Duration, timelines: &mut <ExModel as Model>::OperationTimelines) {
        let a_child = timelines.a.last_before(time);
        let b_child = timelines.b.last_before(time);

        let op = Arc::new(RwLock::new(Complex {
            a_child: a_child.1.get_op(),
            b_child: b_child.1.get_op(),
            output: None,
        }));

        let a_write_node = OperationNode::new(op.clone(), vec![]);
        let b_write_node = OperationNode::new(op, vec![]);

        timelines.a.insert(time, a_write_node);
        timelines.b.insert(time, b_write_node);
    }
}

struct Complex {
    a_child: Arc<dyn Operation<ExModel, AType>>,
    b_child: Arc<dyn Operation<ExModel, BType>>,
    output: Option<ComplexOutput>
}

impl Complex {
    async fn run(&mut self, history: &ExHistory) {
        let a_result = self.a_child.run(history).await;
        let b_result = self.b_child.run(history).await;

        let new_a = *a_result + u32::from_str(&b_result.repeat(2)).unwrap();
        let new_b = (new_a * 2).to_string();

        self.output = Some(ComplexOutput {
            a: new_a.clone(),
            b: new_b.clone()
        });

        let hash = self.history_hash();
        history.a.insert(hash, new_a);
        history.b.insert(hash, new_b);
    }

    fn history_hash(&self) -> u64 {
        let mut state = SwiftDefaultHashBuilder::default().build_hasher();

        TypeId::of::<Complex>().hash(&mut state);

        self.a_child.history_hash().hash(&mut state);
        self.b_child.history_hash().hash(&mut state);

        state.finish()
    }

    fn find_children(&mut self, time: Duration, timelines: &<ExModel as Model>::OperationTimelines) {
        let a_child = timelines.a.last_before(time);
        let b_child = timelines.b.last_before(time);

        self.a_child = a_child.1.get_op();
        self.b_child = b_child.1.get_op();
    }
}

struct ComplexOutput {
    a: u32,
    b: String
}

#[async_trait]
impl Operation<ExModel, BType> for RwLock<Complex> {
    async fn run(&self, history: &ExHistory) -> RwLockReadGuard<String> {
        if let Ok(mut write) = self.try_write() {
            write.run(history).await;
        }

        return RwLockReadGuard::map(self.read().await, |o| &o.output.as_ref().unwrap().b);
    }

    fn history_hash(&self) -> u64 {
        self.try_read().unwrap().history_hash()
    }

    async fn find_children(&self, time: Duration, timelines: &<ExModel as Model>::OperationTimelines) {
        self.write().await.find_children(time, timelines);
    }
}

#[async_trait]
impl Operation<ExModel, AType> for RwLock<Complex> {
    async fn run(&self, history: &ExHistory) -> RwLockReadGuard<u32> {
        if let Ok(mut write) = self.try_write() {
            write.run(history).await;
        }

        return RwLockReadGuard::map(self.read().await, |o| &o.output.as_ref().unwrap().a);
    }

    fn history_hash(&self) -> u64 {
        self.try_read().unwrap().history_hash()
    }

    async fn find_children(&self, time: Duration, timelines: &<ExModel as Model>::OperationTimelines) {
        self.write().await.find_children(time, timelines);
    }
}

#[cfg(test)]
mod tests {
    use crate::duration::Duration;
    use crate::example::{ExModel, JustTheIncrement, MyActivity};
    use crate::Session;

    #[tokio::test]
    async fn do_it() {
        let mut session = Session::<ExModel>::default();

        session.add(Duration(2), JustTheIncrement).await;
        session.add(Duration(1), MyActivity).await;

        let a = *(session.op_timelines.a.last().run(&session.history).await);
        let b = &*session.op_timelines.b.last().run(&session.history).await.to_string();

        let _ = dbg!(a, b);
    }
}

