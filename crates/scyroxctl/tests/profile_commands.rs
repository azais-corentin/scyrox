//! Integration tests for `scyroxctl profile` commands.
//!
//! These run against the live daemon (profiles are daemon-only) and, where a
//! profile is created from current settings or applied, a connected mouse.
//! They restore the prior default profile and delete their fixture via RAII.

mod common;

use common::{
    ConfigGuard, assert_daemon_running, assert_device_connected, parse_json_output,
    scyroxctl_daemon,
};
use serde_json::Value;

/// Fixed fixture name; the daemon slugifies it to a deterministic id, but we
/// always read the real id back from the create output.
const TEST_PROFILE_NAME: &str = "test-profile-scyroxctl-it";

fn list_profiles() -> Value {
    let output = scyroxctl_daemon()
        .args(["-f", "json", "profile", "list"])
        .output()
        .expect("Failed to execute profile list");
    assert!(
        output.status.success(),
        "profile list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_json_output(&output)
}

fn list_contains(id: &str) -> bool {
    list_profiles()
        .as_array()
        .map(|arr| arr.iter().any(|p| p["id"] == id))
        .unwrap_or(false)
}

fn current_default_id() -> Option<String> {
    list_profiles().as_array().and_then(|arr| {
        arr.iter()
            .find(|p| p["is_default"] == true)
            .and_then(|p| p["id"].as_str().map(String::from))
    })
}

/// Creates the fixture profile and returns its id, restoring prior default and
/// deleting the fixture on drop.
struct ProfileGuard {
    id: String,
    prior_default: Option<String>,
}

impl ProfileGuard {
    /// Create the fixture profile and capture the prior default for restoration.
    fn create() -> Self {
        let prior_default = current_default_id();

        let output = scyroxctl_daemon()
            .args(["-f", "json", "profile", "create", TEST_PROFILE_NAME])
            .output()
            .expect("Failed to execute profile create");
        assert!(
            output.status.success(),
            "profile create failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let json = parse_json_output(&output);
        assert_eq!(
            json["name"], TEST_PROFILE_NAME,
            "create output should echo the name"
        );
        let id = json["id"]
            .as_str()
            .expect("create output should contain an id")
            .to_string();

        Self { id, prior_default }
    }
}

impl Drop for ProfileGuard {
    fn drop(&mut self) {
        let _ = scyroxctl_daemon()
            .args(["profile", "delete", &self.id])
            .output();
        if let Some(id) = &self.prior_default {
            let _ = scyroxctl_daemon()
                .args(["profile", "set-default", id])
                .output();
        }
    }
}

#[test]
fn test_profile_create_appears_in_list() {
    assert_daemon_running();
    assert_device_connected();

    let guard = ProfileGuard::create();
    assert!(
        list_contains(&guard.id),
        "created profile {} should appear in profile list",
        guard.id
    );
}

#[test]
fn test_profile_show_returns_created_profile() {
    assert_daemon_running();
    assert_device_connected();

    let guard = ProfileGuard::create();

    let output = scyroxctl_daemon()
        .args(["-f", "json", "profile", "show", &guard.id])
        .output()
        .expect("Failed to execute profile show");
    assert!(
        output.status.success(),
        "profile show failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = parse_json_output(&output);
    assert_eq!(json["id"], guard.id);
    assert_eq!(json["name"], TEST_PROFILE_NAME);
}

#[test]
fn test_profile_set_default_marks_profile() {
    assert_daemon_running();
    assert_device_connected();

    let guard = ProfileGuard::create();

    scyroxctl_daemon()
        .args(["profile", "set-default", &guard.id])
        .assert()
        .success();

    let output = scyroxctl_daemon()
        .args(["-f", "json", "profile", "show", &guard.id])
        .output()
        .expect("Failed to execute profile show");
    assert!(output.status.success());
    let json = parse_json_output(&output);
    assert_eq!(
        json["is_default"], true,
        "profile should report is_default after set-default"
    );
}

#[test]
fn test_profile_delete_removes_from_list() {
    assert_daemon_running();
    assert_device_connected();

    let guard = ProfileGuard::create();
    let id = guard.id.clone();
    assert!(list_contains(&id));

    scyroxctl_daemon()
        .args(["profile", "delete", &id])
        .assert()
        .success();

    assert!(
        !list_contains(&id),
        "deleted profile {id} should no longer appear in profile list"
    );

    // The guard's Drop will attempt another delete (ignored for a missing
    // profile) and restore the prior default.
    drop(guard);
}

#[test]
fn test_profile_show_deleted_fails() {
    assert_daemon_running();
    assert_device_connected();

    let guard = ProfileGuard::create();
    let id = guard.id.clone();

    scyroxctl_daemon()
        .args(["profile", "delete", &id])
        .assert()
        .success();

    scyroxctl_daemon()
        .args(["profile", "show", &id])
        .assert()
        .failure();

    // Drop restores the prior default; its delete of the missing fixture is
    // ignored.
    drop(guard);
}

#[test]
fn test_profile_apply_succeeds() {
    assert_daemon_running();
    assert_device_connected();
    let _config = ConfigGuard::new();

    let guard = ProfileGuard::create();

    scyroxctl_daemon()
        .args(["profile", "apply", &guard.id])
        .assert()
        .success();
}
