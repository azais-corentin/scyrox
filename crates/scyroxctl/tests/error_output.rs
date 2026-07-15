//! Integration test for machine-readable (`--format json`) error output.

mod common;

use common::{assert_daemon_running, scyroxctl_daemon};
use serde_json::Value;

#[test]
fn test_json_error_goes_to_stderr_as_json() {
    // A runtime error only surfaces once the daemon is reachable; without it the
    // CLI would fail earlier for a different reason.
    assert_daemon_running();

    let output = scyroxctl_daemon()
        .args(["-f", "json", "profile", "show", "definitely-not-a-profile"])
        .output()
        .expect("Failed to execute scyroxctl");

    assert!(
        !output.status.success(),
        "showing a missing profile should fail"
    );

    // stdout stays reserved for success payloads.
    assert!(
        output.stdout.is_empty(),
        "no success payload should be written to stdout on error: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let json: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be a JSON object on error");
    assert!(
        json["error"].is_string(),
        "error output should carry a string `error` field: {json}"
    );
}
