#![allow(dead_code)]
#![forbid(long_running_const_eval)]
const CLIENT: u8 = 0;
const SERVER: u8 = 1;

#[hibana_runtime::scope]
async fn retain(_value: &mut u32) -> Result<(), ()> { Ok(()) }
fn escape() -> impl core::future::Future<Output = Result<(), ()>> {
    let mut value = 0;
    retain(&mut value)
}

fn main() {}
