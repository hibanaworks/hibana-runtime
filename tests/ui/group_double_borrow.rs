#![allow(dead_code)]
#![forbid(long_running_const_eval)]

#[hibana_runtime::scope]
async fn example(mut endpoint: hibana::Endpoint<0>) -> Result<(), hibana::EndpointError> {
    #[concurrent]
    let (_first, _second) = (
        endpoint.recv::<hibana::g::Msg<1, u32>>(),
        endpoint.recv::<hibana::g::Msg<2, u32>>(),
    );
    Ok(())
}

fn main() {}
