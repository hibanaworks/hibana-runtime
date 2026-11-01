#![allow(dead_code)]
#![forbid(long_running_const_eval)]
const CLIENT: u8 = 0;
const SERVER: u8 = 1;

#[hibana_runtime::scope]
async fn example() -> Result<(), ()> {
    {
        #[concurrent]
        core::future::ready(Ok::<(), ()>(()));
    }
    Ok(())
}

fn main() {}
