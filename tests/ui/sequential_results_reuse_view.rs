#![allow(dead_code)]
#![forbid(long_running_const_eval)]

#[hibana_runtime::scope]
async fn example(mut endpoint: hibana::Endpoint<0>) -> Result<(), hibana::EndpointError> {

let first = async { async { endpoint.recv::<hibana::g::Msg<1, u32>>().await }; }.await?;

let second = async { async { endpoint.recv::<hibana::g::Msg<2, u32>>().await }; }.await?;
    let _ = (first, second);
    Ok(())
}

fn main() {}
