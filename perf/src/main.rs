use peregrine::reexports::hifitime::TimeScale;
use peregrine::{impl_activity, model, resource, Duration, Session, Time};

model! {
    pub Perf(a, b)
}

resource!(a: u32);
resource!(ref b: String);

struct IncrementA;
impl_activity! { for IncrementA
    @(start) a -> a {
        a += 1;
    }
    Duration::ZERO
}

struct ConvertAToB;
impl_activity! { for ConvertAToB
    @(start) a -> b {
        b = a.to_string()
    }
    Duration::ZERO
}

struct ConvertBToA;
impl_activity! { for ConvertBToA
    @(start) b -> a {
        a = b.parse()?;
    }
    Duration::ZERO
}

fn main() -> peregrine::Result<()> {
    let session = Session::new();

    let plan_start = Time::now()?.to_time_scale(TimeScale::TAI);
    let mut plan = session.new_plan::<Perf>(
        plan_start,
        PerfInitialConditions {
            a: 0,
            b: "".to_string(),
        },
    );

    let mut cursor = plan_start + Duration::from_microseconds(1.0);

    for _ in 0..10_000_000 {
        plan.insert(cursor, IncrementA)?;
        cursor += Duration::from_seconds(1.0);
        plan.insert(cursor, ConvertAToB)?;
        cursor += Duration::from_seconds(1.0);
        plan.insert(cursor, ConvertBToA)?;
        cursor += Duration::from_seconds(1.0);
    }

    println!("built");

    let start = plan_start + Duration::from_seconds(30_000_000.0 - 10.0);
    let result = plan.view::<b>(start..start + Duration::from_seconds(10.0))?;

    dbg!(result);

    Ok(())
}
