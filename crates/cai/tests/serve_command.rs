use std::process::Command;

#[test]
fn serve_command_rejects_a_public_listener_address() {
    let output = Command::new(env!("CARGO_BIN_EXE_cai"))
        .args(["serve", "--listen", "0.0.0.0:8080"])
        .output()
        .expect("binary must start");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("loopback"));
}
