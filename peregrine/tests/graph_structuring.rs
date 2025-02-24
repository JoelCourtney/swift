mod util;

use peregrine::*;
use util::*;

#[tokio::test]
async fn basic_insertion() -> Result<()> {
    let session = Session::new();
    let mut plan = init_plan(&session);

    plan.insert(seconds(0), IncrementA)?;
    plan.insert(seconds(1), SetBToA)?;

    assert_eq!(1, plan.sample::<a>(seconds(1)).await?);
    assert_eq!(1, plan.sample::<b>(seconds(1)).await?);

    Ok(())
}

#[tokio::test]
async fn longer_chain() -> Result<()> {
    let session = Session::new();
    let mut plan = init_plan(&session);

    for i in 0..100 {
        plan.insert(seconds(4 * i), IncrementA)?;
        plan.insert(seconds(4 * i + 1), SetBToA)?;
        plan.insert(seconds(4 * i + 2), IncrementB)?;
        plan.insert(seconds(4 * i + 3), SetAToB)?;
    }

    assert_eq!(5, plan.sample::<a>(seconds(8)).await?);
    assert_eq!(4, plan.sample::<b>(seconds(8)).await?);

    assert_eq!(200, plan.sample::<a>(seconds(400)).await?);
    assert_eq!(200, plan.sample::<b>(seconds(400)).await?);

    Ok(())
}

#[tokio::test]
async fn backward_insertion() -> Result<()> {
    let session = Session::new();
    let mut plan = init_plan(&session);

    plan.insert(seconds(2), IncrementA)?;
    plan.insert(seconds(1), SetAToB)?;
    plan.insert(seconds(0), IncrementB)?;

    assert_eq!(2, plan.sample::<a>(seconds(2)).await?);

    Ok(())
}

#[tokio::test]
async fn out_of_order_insertion() -> Result<()> {
    let session = Session::new();
    let mut plan = init_plan(&session);

    plan.insert(seconds(1), SetBToA)?;
    plan.insert(seconds(0), IncrementA)?;
    plan.insert(seconds(3), SetAToB)?;
    plan.insert(seconds(2), IncrementB)?;

    assert_eq!(2, plan.sample::<a>(seconds(3)).await?);

    Ok(())
}

#[tokio::test]
async fn basic_removal() -> Result<()> {
    let session = Session::new();
    let mut plan = init_plan(&session);

    plan.insert(seconds(0), IncrementA)?;
    plan.insert(seconds(1), SetBToA)?;
    let id = plan.insert(seconds(2), IncrementB)?;
    plan.insert(seconds(3), SetAToB)?;

    assert_eq!(2, plan.sample::<a>(seconds(3)).await?);

    plan.remove(id)?;

    assert_eq!(1, plan.sample::<a>(seconds(3)).await?);

    Ok(())
}
