#![allow(dead_code)]
#![forbid(long_running_const_eval)]
const CLIENT: u8 = 0;
const SERVER: u8 = 1;

struct Resource<'a>(&'a mut u32);
#[hibana_runtime::scope]
async fn retain(resource: Resource) -> Result<(), ()> {
    *resource.0 += 1;
    Ok(())
}
fn escape() -> impl core::future::Future<Output = Result<(), ()>> {
    let mut value = 0;
    retain(Resource(&mut value))
}

fn main() {}
