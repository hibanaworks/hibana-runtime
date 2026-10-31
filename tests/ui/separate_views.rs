#![allow(dead_code)]
#![forbid(long_running_const_eval)]
const CLIENT: u8 = 0;
const SERVER: u8 = 1;

#[hibana_runtime::localside]
async fn example(mut a: hibana::Endpoint<CLIENT>, mut b: hibana::Endpoint<SERVER>) -> Result<(), hibana::EndpointError> {
    async {
        async { a.send::<hibana::g::Msg<1, u32>>(&7).await };
        async { b.send::<hibana::g::Msg<2, u32>>(&8).await };
        Ok(())
    }.await?;
    a.send::<hibana::g::Msg<3, u32>>(&9).await
}

fn main() {}
