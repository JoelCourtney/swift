use swift::exec::{ExecEnvironment, SendBump, EXECUTOR};
use swift::history::{CopyHistory, DerefHistory};
use swift::{activity, model, Duration, Epoch, Model, Plan, Resource};

model! {
    pub Perf {
        a: A,
        b: B
    }
}

pub enum A {}
impl<'h> Resource<'h> for A {
    const PIECEWISE_CONSTANT: bool = true;
    type Read = u32;
    type Write = u32;
    type History = CopyHistory<'h, A>;
}

pub enum B {}
impl<'h> Resource<'h> for B {
    const PIECEWISE_CONSTANT: bool = true;
    type Read = &'h str;
    type Write = String;
    type History = DerefHistory<'h, B>;
}

struct IncrementA;
activity! {
    for IncrementA {
        @(start) a: A -> a {
            a += 1;
        }
    }
}

struct ConvertAToB;
activity! {
    for ConvertAToB {
        @(start) a: A -> b: B {
            b = a.to_string()
        }
    }
}

struct ConvertBToA;
activity! {
    for ConvertBToA {
        @(start) b: B -> a: A {
            a = b.parse().unwrap();
        }
    }
}

fn main() {
    let bump = SendBump::new();
    let histories = PerfHistories::default();
    let plan_start = Epoch::now().unwrap();
    let mut plan = Perf::new_plan(
        plan_start,
        PerfInitialConditions {
            a: 0,
            b: "".to_string(),
        },
        &bump,
    );

    let offset = Duration::from_microseconds(1.0);

    for i in 0..10000000 {
        plan.insert(
            plan_start + offset + 3 * i * Duration::from_seconds(1.0),
            IncrementA,
        );
        plan.insert(
            plan_start + offset + 3 * i * Duration::from_seconds(1.0) + Duration::from_seconds(1.0),
            ConvertAToB,
        );
        plan.insert(
            plan_start + offset + 3 * i * Duration::from_seconds(1.0) + Duration::from_seconds(2.0),
            ConvertBToA,
        );
    }

    let futures_bump = SendBump::new();
    let future = plan
        .a_operation_timeline
        .last()
        .read(&histories, ExecEnvironment::new(&futures_bump));

    let result = futures_lite::future::block_on(EXECUTOR.run(future));

    println!("{}", result.1);
}
