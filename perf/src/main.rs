use serde::{Deserialize, Serialize};
use swift::reexports::tokio;
use swift::{impl_activity, model, Duration, Durative, Session};

model! {
    pub struct Perf {
        a: u32,
        b: String
    }
}

#[derive(Serialize, Deserialize, Durative, Clone)]
pub struct ConvertAToB;

impl_activity! {
    for ConvertAToB in Perf {
        start => {
            :b = ?a.to_string();
        }
    }
}

#[derive(Serialize, Deserialize, Durative, Clone)]
pub struct ConvertBToA;

impl_activity! {
    for ConvertBToA in Perf {
        start => {
            :a = ?b.parse().unwrap();
        }
    }
}

#[derive(Serialize, Deserialize, Durative, Clone)]
pub struct IncrementA;

impl_activity! {
    for IncrementA in Perf {
        start => {
            ?:a += 1;
        }
    }
}

#[tokio::main]
async fn main() {
    let mut session = Session::<Perf>::default();

    for i in 1..10000 {
        session.add(Duration(3 * i), IncrementA).await;
        session.add(Duration(3 * i + 1), ConvertAToB).await;
        session.add(Duration(3 * i + 2), ConvertBToA).await;
    }

    let a = &*session.op_timelines.a.last().run().await.to_string();

    let b = &*session.op_timelines.b.last().run().await.to_string();

    dbg!(a, b);

    drop(session);

    println!("hi");
}
