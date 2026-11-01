#![allow(dead_code)]
#![forbid(long_running_const_eval)]
#[hibana_runtime::scope]
async fn example(mut endpoint: hibana::Endpoint<0>) -> Result<(), hibana::EndpointError> {

for view in [&mut endpoint, &mut endpoint] { async move {
        view.send::<hibana::g::Msg<1, u32>>(&7).await?;
    } ; }
    Ok(())
}
fn main() {}
