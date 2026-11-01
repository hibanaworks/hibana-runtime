#![allow(dead_code)]
#![forbid(long_running_const_eval)]
const CLIENT: u8 = 0;
const SERVER: u8 = 1;

#[hibana_runtime::scope]
async fn example(mut a: hibana::Endpoint<CLIENT>) -> Result<(), hibana::EndpointError> {
    async { a.send::<hibana::g::Msg<1, u32>>(&7).await };
    drop(a);
    Ok(())
}

fn main() {}
