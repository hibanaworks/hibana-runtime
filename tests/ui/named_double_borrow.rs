#![allow(dead_code)]
#![forbid(long_running_const_eval)]
const CLIENT: u8 = 0;
const SERVER: u8 = 1;

#[hibana_runtime::scope]
async fn send(endpoint: &mut hibana::Endpoint<CLIENT>) -> Result<(), hibana::EndpointError> {
    endpoint.send::<hibana::g::Msg<1, u32>>(&7).await
}
#[hibana_runtime::scope]
async fn example(mut a: hibana::Endpoint<CLIENT>) -> Result<(), hibana::EndpointError> {

async { (send(&mut a)).await };

async { (send(&mut a)).await };
    Ok(())
}

fn main() {}
