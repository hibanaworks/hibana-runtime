#![allow(dead_code)]
#![forbid(long_running_const_eval)]

#[hibana_runtime::scope]
async fn example() -> Result<(), ()> {
    let work = async { Ok::<_, ()>(7) };
    #[concurrent]
    let (_first, _second) = (work, work);
    Ok(())
}

fn main() {}
