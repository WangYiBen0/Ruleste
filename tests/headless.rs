//! Headless end-to-end runs driven by the `ruleste` binary itself.
//!
//! These exercise the same path an integration test can use without a window:
//! `ruleste --headless-*` loads the real map, the real Wasm plugins and the
//! real physics, then prints a small report on stdout. The binary is skipped
//! when it has not been built, and the wasm plugins are skipped when they are
//! absent, so a fresh checkout can still run `cargo test`.
//!
//! Run the same thing by hand:
//!
//! ```text
//! cargo run -- --headless-frames=120 \
//!     --headless-input=60:move_right \
//!     --plugin-path=target/wasm32-unknown-unknown/release \
//!     maps/Celeste/0-Intro.bin
//! ```

use std::path::PathBuf;
use std::process::Command;

/// The `ruleste` binary next to the integration-test executable
/// (`target/<profile>/deps/..`).
fn binary() -> PathBuf {
    let mut path = std::env::current_exe().expect("test executable path");
    path.pop(); // deps/
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("ruleste")
}

/// The wasm plugin dir, mirroring the other integration tests.
fn plugin_dir() -> PathBuf {
    std::env::var_os("RULESTE_PLUGIN_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/wasm32-unknown-unknown/release"))
}

fn has_plugins() -> bool {
    std::fs::read_dir(plugin_dir())
        .map(|it| {
            it.flatten()
                .any(|e| e.path().extension().is_some_and(|e| e == "wasm"))
        })
        .unwrap_or(false)
}

/// The intro map, which ships with the repository layout.
fn intro_map() -> PathBuf {
    PathBuf::from("maps/Celeste/0-Intro.bin")
}

/// Runs the binary headlessly and returns its stdout, or `None` when the
/// environment cannot support the run.
fn run_headless(extra: &[&str]) -> Option<String> {
    let bin = binary();
    if !bin.exists() || !has_plugins() || !intro_map().exists() {
        eprintln!("skipping: build the client and the wasm plugins first");
        return None;
    }
    let mut cmd = Command::new(&bin);
    cmd.arg("--headless-frames=30")
        .arg("--log-level=off")
        .arg(format!("--plugin-path={}", plugin_dir().display()));
    for a in extra {
        cmd.arg(a);
    }
    cmd.arg(intro_map());
    let out = cmd.output().expect("run ruleste headless");
    assert!(
        out.status.success(),
        "headless run failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Reads `key=value` out of the headless report. The report is several
/// `headless: ...` lines, so every line is searched.
fn field(report: &str, key: &str) -> String {
    let prefix = format!("{key}=");
    report
        .lines()
        .filter_map(|l| l.strip_prefix("headless: "))
        .find_map(|l| l.split_whitespace().find_map(|kv| kv.strip_prefix(&prefix)))
        .unwrap_or_else(|| panic!("field {key:?} missing from report:\n{report}"))
        .to_string()
}

#[test]
fn headless_run_reports_player_state() {
    let Some(report) = run_headless(&[]) else {
        return;
    };
    assert!(report.contains("headless: frames=30"), "report:\n{report}");
    assert!(report.contains("player start="), "report:\n{report}");
    assert!(field(&report, "entities").parse::<u32>().is_ok());
}

#[test]
fn headless_scripted_input_moves_the_player_right() {
    let Some(report) = run_headless(&["--headless-frames=90", "--headless-input=60:move_right"])
    else {
        return;
    };
    // Holding `move_right` for 60 frames must displace her along +x; the delta
    // is printed as a tuple, so check the horizontal component explicitly.
    let delta = field(&report, "delta");
    let x: f32 = delta
        .trim_start_matches('(')
        .split(',')
        .next()
        .unwrap_or_default()
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("cannot read delta x from {delta:?}"));
    assert!(
        x > 1.0,
        "expected forward motion, got delta={delta} in\n{report}"
    );
}

#[test]
fn log_level_off_silences_startup_banners() {
    let Some(bin) = binary().exists().then(binary) else {
        return;
    };
    if !has_plugins() || !intro_map().exists() {
        eprintln!("skipping: build the client and the wasm plugins first");
        return;
    }
    let out = Command::new(&bin)
        .arg("--headless-frames=2")
        .arg("--log-level=off")
        .arg(format!("--plugin-path={}", plugin_dir().display()))
        .arg(intro_map())
        .output()
        .expect("run ruleste headless");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("[info"),
        "--log-level=off must silence the info banners, stderr:\n{stderr}"
    );
    // The report still reaches stdout, which is what a script reads.
    assert!(String::from_utf8_lossy(&out.stdout).contains("headless: frames=2"));
}

#[test]
fn invalid_log_level_exits_with_a_usage_error() {
    let bin = binary();
    if !bin.exists() {
        eprintln!("skipping: build the client first");
        return;
    }
    let out = Command::new(&bin)
        .arg("--log-level=loud")
        .output()
        .expect("run ruleste");
    assert_eq!(
        out.status.code(),
        Some(2),
        "stderr: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
}
