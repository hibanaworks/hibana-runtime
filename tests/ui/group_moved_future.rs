#![allow(dead_code)]
#![forbid(long_running_const_eval)]

#[hibana_runtime::scope]
async fn example() -> Result<(), ()> {
    let work = async { Ok::<_, ()>(7) };

let (_first, _second) = (async { async { (work).await };
async { (work).await }; }).await?;
    Ok(())
}

fn main() {}
