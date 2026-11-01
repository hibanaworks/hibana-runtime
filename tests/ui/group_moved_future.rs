#![allow(dead_code)]
#![forbid(long_running_const_eval)]

#[hibana_runtime::localside]
async fn example() -> Result<(), ()> {
    let work = async { Ok::<_, ()>(7) };
    #[localside]
    let _values = (work, work);
    Ok(())
}

fn main() {}
