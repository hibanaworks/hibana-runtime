#![cfg(not(miri))]

use std::{fs, path::Path, process::Command};

#[test]
fn compiler_checks_exclusive_views_moves_and_scoped_borrows() -> std::io::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = root.join("validation/ui");
    fs::create_dir_all(project.join("src/bin"))?;
    fs::write(
        project.join("Cargo.toml"),
        format!(
            "[package]\nname=\"hibana-runtime-ui\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\nhibana=\"=0.9.9\"\nhibana-runtime={{path={:?}}}\n[lints.rust]\nlong_running_const_eval=\"forbid\"\n",
            root
        ),
    )?;
    let cases = [
        ("separate_views", None),
        ("same_view_twice", Some("E0499")),
        ("array_double_borrow", Some("E0499")),
        ("sequential_results_reuse_view", None),
        ("group_double_borrow", Some("E0499")),
        ("group_moved_future", Some("E0382")),
        ("same_future_twice", Some("E0382")),
        ("move_borrowed_view", Some("E0505")),
        ("named_double_borrow", Some("E0499")),
        ("named_moved_future", Some("E0382")),
        (
            "plain_block_is_not_an_async_boundary",
            Some("concurrent declarations belong to an async body; ordinary blocks never wait"),
        ),
        ("elided_input_lifetime_cannot_escape", Some("E0597")),
        ("unused_input_lifetime_cannot_escape", Some("E0597")),
        ("role_mismatch", Some("E0308")),
        (
            "marker_arguments",
            Some("a concurrent declaration takes no arguments"),
        ),
    ];
    for (name, expected) in cases {
        fs::copy(
            root.join("tests/ui").join(format!("{name}.rs")),
            project.join("src/bin").join(format!("{name}.rs")),
        )?;
        let output = Command::new("cargo")
            .args([
                "+stable",
                "check",
                "--color=never",
                "--message-format=short",
                "-j",
                "1",
                "--bin",
                name,
                "--manifest-path",
            ])
            .arg(project.join("Cargo.toml"))
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_TARGET_DIR", root.join("target/ui"))
            .output()?;
        let log = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        fs::write(project.join(format!("{name}.log")), &log)?;
        let errors: Vec<_> = log
            .lines()
            .filter(|line| line.contains(": error"))
            .collect();
        match expected {
            None => assert!(output.status.success(), "{name}: {log}"),
            Some(expected) => {
                assert!(!output.status.success(), "{name} unexpectedly compiled");
                assert_eq!(errors.len(), 1, "{name}: {log}");
                assert!(
                    errors[0].contains(expected),
                    "{name}: expected {expected}: {log}"
                );
            }
        }
        println!("{name}: {}", expected.unwrap_or("accepted"));
    }
    Ok(())
}
