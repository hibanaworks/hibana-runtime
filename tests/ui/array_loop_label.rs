#![allow(dead_code)]
#![forbid(long_running_const_eval)]

#[hibana_runtime::scope]
async fn example() -> Result<(), ()> {
    'participants: for value in [1, 2] {
        async move { let _ = value; };
    }
    Ok(())
}
fn main() {}
