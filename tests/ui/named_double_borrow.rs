#![allow(dead_code)]
#![forbid(long_running_const_eval)]
const CLIENT: u8 = 0;
const SERVER: u8 = 1;

#[hibana_runtime::localside]
async fn send(endpoint: &mut hibana::Endpoint<CLIENT>) -> Result<(), hibana::EndpointError> {
    endpoint.send::<hibana::g::Msg<1, u32>>(&7).await
}
#[hibana_runtime::localside]
async fn example(mut a: hibana::Endpoint<CLIENT>) -> Result<(), hibana::EndpointError> {
    #[localside]
    send(&mut a);
    #[localside]
    send(&mut a);
    Ok(())
}

fn main() {}
