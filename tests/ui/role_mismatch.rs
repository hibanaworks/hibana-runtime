#![allow(dead_code)]
#![forbid(long_running_const_eval)]
const CLIENT: u8 = 0;
const SERVER: u8 = 1;

#[hibana_runtime::localside]
async fn serve(_server: hibana::Endpoint<SERVER>) -> Result<(), ()> { Ok(()) }
#[hibana_runtime::localside]
async fn example(client: hibana::Endpoint<CLIENT>) -> Result<(), ()> {
    serve(client).await
}

fn main() {}
