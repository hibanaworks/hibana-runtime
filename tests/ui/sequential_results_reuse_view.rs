#![allow(dead_code)]
#![forbid(long_running_const_eval)]

#[hibana_runtime::scope]
async fn example(mut endpoint: hibana::Endpoint<0>) -> Result<(), hibana::EndpointError> {
    #[concurrent]
    let first = endpoint.recv::<hibana::g::Msg<1, u32>>();
    #[concurrent]
    let second = endpoint.recv::<hibana::g::Msg<2, u32>>();
    let _ = (first, second);
    Ok(())
}

fn main() {}
