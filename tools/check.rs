use std::{error::Error, fs, path::Path, process::Command};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn capture(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        return Err(format!("{program}: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn sources() -> Result<String> {
    let names = capture(
        "git",
        &["ls-files", "--cached", "--others", "--exclude-standard"],
    )?;
    let mut names: Vec<_> = names
        .lines()
        .filter(|name| Path::new(name).is_file())
        .map(str::to_owned)
        .collect();
    let core = capture(
        "git",
        &["-C", "../hibana", "ls-files", "--cached", "--others", "--exclude-standard"],
    )?;
    names.extend(core.lines().map(|name| format!("../hibana/{name}")).filter(|name| Path::new(name).is_file()));
    names.sort_unstable();
    names.dedup();
    let names: Vec<_> = names.iter().map(String::as_str).collect();
    capture("shasum", &[&["-a", "256", "--"], &names[..]].concat())
}

fn check(name: &str, program: &str, args: &[&str]) -> Result<String> {
    println!("Checking {name}");
    let mut command = Command::new(program);
    command.args(args);
    if name.starts_with("miri") {
        command.env("MIRIFLAGS", "-Zmiri-strict-provenance");
    }
    let output = command.output()?;
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(format!("validation/{name}.log"), &log)?;
    if !output.status.success() {
        return Err(format!("{name}: {}\n{log}", output.status).into());
    }
    Ok(log)
}

fn main() -> Result<()> {
    fs::create_dir_all("validation")?;
    let before = sources()?;
    fs::write("validation/source-before.sha256", &before)?;
    let versions = format!(
        "{}{}{}core={}",
        capture("rustc", &["+stable", "--version"])?,
        capture("cargo", &["+stable", "--version"])?,
        capture("git", &["rev-parse", "HEAD"])?,
        capture("git", &["-C", "../hibana", "rev-parse", "HEAD"])?
    );
    fs::write("validation/versions.txt", versions)?;
    fs::write("validation/runtime-dirty.patch", capture("git", &["diff", "HEAD", "--binary"])?)?;
    fs::write("validation/core-dirty.patch", capture("git", &["-C", "../hibana", "diff", "HEAD", "--binary"])?)?;
    check(
        "format",
        "cargo",
        &["+stable", "fmt", "--all", "--", "--check"],
    )?;
    let features = if cfg!(target_os = "macos") {
        "host"
    } else {
        ""
    };
    check(
        "clippy",
        "cargo",
        &[
            "+stable",
            "clippy",
            "--all-targets",
            "--workspace",
            "--locked",
            "--features",
            features,
            "-j",
            "1",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    check(
        "tests",
        "cargo",
        &[
            "+stable",
            "test",
            "--all-targets",
            "--workspace",
            "--locked",
            "--features",
            features,
            "-j",
            "1",
            "--",
            "--test-threads=1",
        ],
    )?;
    check(
        "docs",
        "cargo",
        &[
            "+stable",
            "test",
            "--doc",
            "--locked",
            "--features",
            features,
            "-j",
            "1",
        ],
    )?;
    for example in ["localsides", "images", "registration_pressure"] {
        check(
            example,
            "cargo",
            &[
                "+stable",
                "run",
                "--example",
                example,
                "--locked",
                "-j",
                "1",
            ],
        )?;
    }
    check(
        "thumbv6m",
        "cargo",
        &[
            "+stable",
            "check",
            "--lib",
            "--target",
            "thumbv6m-none-eabi",
            "--no-default-features",
            "--locked",
            "-j",
            "1",
        ],
    )?;
    check(
        "pico-link",
        "cargo",
        &[
            "+stable",
            "rustc",
            "--example",
            "pico",
            "--release",
            "--target",
            "thumbv6m-none-eabi",
            "--locked",
            "-j",
            "1",
            "--",
            "-C",
            "panic=abort",
            "-C",
            "link-arg=-Tpico/link.x",
        ],
    )?;
    check(
        "lean",
        "lean",
        &["+leanprover/lean4:v4.34.1", "proofs/Scheduler.lean"],
    )?;
    let z3 = check("z3", "z3", &["proofs/scheduler.smt2"])?;
    let answers: Vec<_> = z3
        .lines()
        .filter(|line| ["sat", "unsat", "unknown"].contains(line))
        .collect();
    assert_eq!(answers, ["sat", "unsat"].repeat(8));
    check(
        "miri",
        "cargo",
        &[
            "+nightly-2026-10-12",
            "miri",
            "test",
            "--test",
            "runtime",
            "--test",
            "localside",
            "--test",
            "results",
            "--test",
            "scope_values",
            "--test",
            "array_configuration",
            "--test",
            "entry",
            "--test",
            "no_alloc",
            "--test",
            "placement",
            "--locked",
            "-j",
            "1",
        ],
    )?;
    check(
        "miri-localsides",
        "cargo",
        &[
            "+nightly-2026-10-12",
            "miri",
            "run",
            "--example",
            "localsides",
            "--locked",
            "-j",
            "1",
        ],
    )?;
    let after = sources()?;
    fs::write("validation/source-after.sha256", &after)?;
    if before != after {
        return Err("source changed during validation".into());
    }
    println!("Runtime checks passed on the same source.");
    Ok(())
}
