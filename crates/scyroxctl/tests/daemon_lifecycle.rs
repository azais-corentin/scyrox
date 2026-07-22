#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use predicates::prelude::*;

use common::scyroxctl_raw;

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new() -> Self {
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "scyroxctl-daemon-lifecycle-tests-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self { path }
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn restart_releases_captured_output_while_spawned_process_runs() {
    let test_dir = TestDir::new();
    let bin_dir = test_dir.path.join("bin");
    fs::create_dir(&bin_dir).unwrap();

    let setsid_path = bin_dir.join("setsid");
    fs::write(&setsid_path, "#!/bin/sh\nsleep 5\n").unwrap();
    let mut permissions = fs::metadata(&setsid_path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&setsid_path, permissions).unwrap();

    let runtime_dir = test_dir.path.join("runtime");
    fs::create_dir(&runtime_dir).unwrap();

    let inherited_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin_dir];
    paths.extend(std::env::split_paths(&inherited_path));
    let path = std::env::join_paths(paths).unwrap();

    let started = Instant::now();
    scyroxctl_raw()
        .args(["daemon", "restart"])
        .env("PATH", path)
        .env("XDG_RUNTIME_DIR", runtime_dir)
        .timeout(Duration::from_secs(3))
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Warning: Could not verify daemon is running",
        ));

    assert!(
        started.elapsed() < Duration::from_secs(3),
        "daemon restart kept captured output open"
    );
}
