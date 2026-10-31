#![allow(dead_code)]
#![forbid(long_running_const_eval)]
const CLIENT: u8 = 0;
const SERVER: u8 = 1;

#[hibana_runtime::localside]
async fn example() -> Result<(), ()> {
    let future = async { Ok::<(), ()>(()) };
    async { future.await };
    async { future.await };
    Ok(())
}

fn main() {}
