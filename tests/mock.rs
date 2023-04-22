use nix::{
    sys::signal::{self, kill, Signal},
    unistd::Pid,
};
use std::{
    env,
    path::PathBuf,
    process::{ExitStatus, Stdio},
    time::Duration,
};
use tokio::process::{self, Command};

#[tokio::test]
async fn test_binary_able_to_start_up() {
    let terrashine_bin = env!("CARGO_BIN_EXE_terrashine");
    let child = Command::new(terrashine_bin)
        .env("RUST_LOG", "info")
        .arg("--s3-bucket-name=terrashine")
        .arg("--s3-endpoint=http://localhost:9000")
        .arg("--http-redirect-url=https://localhost:9443/")
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn");

    tokio::time::sleep(Duration::from_secs(1)).await;
    kill(
        Pid::from_raw(child.id().unwrap().try_into().unwrap()),
        Signal::SIGTERM,
    )
    .expect("Could not send SIGINT signal");

    let result = child
        .wait_with_output()
        .await
        .expect("PID could not be waited on?");
    assert!(result.status.success());
}
