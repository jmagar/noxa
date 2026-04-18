mod support;

use support::McpProcessHarness;

#[test]
fn server_starts_and_waits_for_stdio_client() {
    let mut harness = McpProcessHarness::spawn().expect("spawn noxa-mcp");
    harness.assert_running().expect("server should stay alive");

    assert!(
        harness.stdin_mut().is_some(),
        "stdin pipe should be present"
    );
    assert!(
        harness.stdout_mut().is_some(),
        "stdout pipe should be present"
    );
}
