#![allow(dead_code)]
#![forbid(long_running_const_eval)]
const CLIENT: u8 = 0;
const SERVER: u8 = 1;

#[hibana_runtime::localside]
async fn example() -> Result<(), ()> {
    let future = async { Ok::<(), ()>(()) };
    #[localside]
    future;
    #[localside]
    future;
    Ok(())
}

fn main() {}
