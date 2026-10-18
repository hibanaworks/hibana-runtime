#!/usr/bin/env python3
"""Build positive and negative examples; assert the actual Rust borrow errors."""
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent

CASES = {
    "separate_views": (None, '''
#[hibana_runtime::localside]
async fn example(mut a: hibana::Endpoint<'_, 0>, mut b: hibana::Endpoint<'_, 1>) -> Result<(), hibana::EndpointError> {
    async {
        async { a.send::<hibana::g::Msg<1, u32>>(&7).await };
        async { b.send::<hibana::g::Msg<2, u32>>(&8).await };
        Ok(())
    }.await?;
    a.send::<hibana::g::Msg<3, u32>>(&9).await
}
'''),
    "same_view_twice": ("E0499", '''
#[hibana_runtime::localside]
async fn example(mut a: hibana::Endpoint<'_, 0>) -> Result<(), hibana::EndpointError> {
    async { a.send::<hibana::g::Msg<1, u32>>(&7).await };
    async { a.send::<hibana::g::Msg<2, u32>>(&8).await };
    Ok(())
}
'''),
    "same_future_twice": ("E0382", '''
#[hibana_runtime::localside]
async fn example() -> Result<(), ()> {
    let future = async { Ok::<(), ()>(()) };
    async { future.await };
    async { future.await };
    Ok(())
}
'''),
    "move_borrowed_view": ("E0505", '''
#[hibana_runtime::localside]
async fn example(mut a: hibana::Endpoint<'_, 0>) -> Result<(), hibana::EndpointError> {
    async { a.send::<hibana::g::Msg<1, u32>>(&7).await };
    drop(a);
    Ok(())
}
'''),
    "named_double_borrow": ("E0499", '''
async fn send(endpoint: &mut hibana::Endpoint<'_, 0>) -> Result<(), hibana::EndpointError> {
    endpoint.send::<hibana::g::Msg<1, u32>>(&7).await
}
#[hibana_runtime::localside]
async fn example(mut a: hibana::Endpoint<'_, 0>) -> Result<(), hibana::EndpointError> {
    #[localside]
    send(&mut a);
    #[localside]
    send(&mut a);
    Ok(())
}
'''),
    "named_moved_future": ("E0382", '''
#[hibana_runtime::localside]
async fn example() -> Result<(), ()> {
    let future = async { Ok::<(), ()>(()) };
    #[localside]
    future;
    #[localside]
    future;
    Ok(())
}
'''),
    "plain_block_is_not_an_async_boundary": ("localside declarations belong to an async body; ordinary blocks never wait", '''
#[hibana_runtime::localside]
async fn example() -> Result<(), ()> {
    {
        #[localside]
        core::future::ready(Ok::<(), ()>(()));
    }
    Ok(())
}
'''),
    "marker_arguments": ("a localside declaration takes no arguments", '''
#[hibana_runtime::localside]
async fn example() -> Result<(), ()> {
    #[localside(role = 0)]
    core::future::ready(Ok::<(), ()>(()));
    Ok(())
}
'''),

}


def main():
    checkout = ROOT / "validation/ui"
    bins = checkout / "src/bin"
    bins.mkdir(parents=True, exist_ok=True)
    (checkout / "Cargo.toml").write_text('''[package]
name = "localside-ui"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
hibana = "=0.9.9"
hibana-runtime = { path = ''' + json.dumps(str(ROOT)) + " }\n")
    for name, (_, source) in CASES.items():
        (bins / f"{name}.rs").write_text("#![allow(dead_code)]\n" + source + "\nfn main() {}\n")
    env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / "target/ui"), CARGO_INCREMENTAL="0")
    for name, (expected, _) in CASES.items():
        result = subprocess.run(["cargo", "+stable", "build", "--manifest-path", str(checkout / "Cargo.toml"),
                                 "--bin", name, "--message-format=json", "-j", "1"],
                                env=env, text=True, capture_output=True)
        errors = []
        for line in result.stdout.splitlines():
            message = json.loads(line)
            if message.get("reason") == "compiler-message" and message["message"]["level"] == "error":
                errors.append(message["message"])
        (checkout / f"{name}.log").write_text(result.stdout + result.stderr)
        codes = [error["code"]["code"] for error in errors if error.get("code")]
        if expected is None:
            matches = result.returncode == 0
        elif expected.startswith("E"):
            matches = result.returncode != 0 and codes == [expected]
        else:
            matches = result.returncode != 0 and len(errors) == 1 and expected in errors[0]["message"]
        if not matches:
            raise SystemExit(f"{name}: expected {expected}, got {codes}\n{result.stderr}\n" +
                             "\n".join(error["rendered"] for error in errors))
        print(f"{name}: {expected or 'build passed'}", flush=True)


if __name__ == "__main__":
    main()
