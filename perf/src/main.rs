use peregrine::exec::SyncBump;
use peregrine::{impl_activity, model, Duration, Plan, Resource, Time};
use peregrine::{CopyHistory, DerefHistory};

model! {
    pub Perf {
        a: A,
        b: B
    }
}

#[derive(Debug)]
pub enum A {}
impl<'h> Resource<'h> for A {
    const STATIC: bool = true;
    type Read = u32;
    type Write = u32;
    type History = CopyHistory<'h, A>;
}

#[derive(Debug)]
pub enum B {}
impl<'h> Resource<'h> for B {
    const STATIC: bool = true;
    type Read = &'h str;
    type Write = String;
    type History = DerefHistory<'h, B>;
}

struct IncrementA;
impl_activity! { for IncrementA
    @(start) a: A -> a {
        a += 1;
    }
    Duration::ZERO
}

struct ConvertAToB;
impl_activity! { for ConvertAToB
    @(start) a: A -> b: B {
        b = a.to_string()
    }
    Duration::ZERO
}

struct ConvertBToA;
impl_activity! { for ConvertBToA
    @(start) b: B -> a: A {
        a = b.parse().unwrap();
    }
    Duration::ZERO
}

fn main() {
    let bump = SyncBump::new();
    let histories = PerfHistories::default();
    let plan_start = Time::now().unwrap();
    let mut plan = Plan::<Perf>::new(
        &bump,
        plan_start,
        PerfInitialConditions {
            a: 0,
            b: "".to_string(),
        },
    );

    let offset = Duration::from_microseconds(1.0);

    for i in 0..1_000_000 {
        plan.insert(
            plan_start + offset + Duration::from_seconds(1.0) * 3 * i,
            IncrementA,
        );
        plan.insert(
            plan_start + offset + Duration::from_seconds(1.0) * 3 * i + Duration::from_seconds(1.0),
            ConvertAToB,
        );
        plan.insert(
            plan_start + offset + Duration::from_seconds(1.0) * 3 * i + Duration::from_seconds(2.0),
            ConvertBToA,
        );
    }

    println!("built");

    let start = plan_start + Duration::from_seconds(3_000_000.0 - 10.0);
    let result = plan.view::<B>(start..start + Duration::from_seconds(10.0), &histories);

    dbg!(result);
}
