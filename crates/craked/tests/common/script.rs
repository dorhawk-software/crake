//! A fake openconnect's script with the paths it needs written in: no
//! environment shared between tests running at once (setting one is
//! unsafe, and racy, with the test harness's threads).

/// `script` with the hook, and the file it notes its calls in.
pub fn scripted(script: &str, dir: &std::path::Path) -> String {
    let calls = dir.join("calls");
    let calls = calls.to_string_lossy();
    script
        .replace("$CRAKE_TEST_HOOK", env!("CARGO_BIN_EXE_crake-hook"))
        .replace("$CRAKE_TEST_CALLS", &calls)
        .replace("$CRAKE_TEST_HANDOVER", &calls)
}
