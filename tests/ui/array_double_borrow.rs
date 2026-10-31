#![allow(dead_code)]
#![forbid(long_running_const_eval)]
#[hibana_runtime::localside]
async fn example(mut endpoint: hibana::Endpoint<0>) -> Result<(), hibana::EndpointError> {
    #[localside]
    for view in [&mut endpoint, &mut endpoint] {
        view.send::<hibana::g::Msg<1, u32>>(&7).await?;
    }
    Ok(())
}
fn main() {}
