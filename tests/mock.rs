mod common;
use common::images::postgres::Postgres;
use nix::{
    sys::signal::{kill, Signal},
    unistd::Pid,
};
use sqlx::PgPool;
use std::{env, process::Stdio};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};


#[sqlx::test]
async fn test_binary_able_to_start_up() {
    let terrashine_bin = env!("CARGO_BIN_EXE_terrashine");
    let options = db.connect_options();

    let mut child = Command::new(terrashine_bin)
        .env("RUST_LOG", "info")
        .arg("--s3-bucket-name=terrashine")
        .arg("--s3-endpoint=http://localhost:9000")
        .arg("--http-redirect-url=https://localhost:9443/")
        .env(
            "TERRASHINE_DATABASE_URL",
            format!("postgresql://postgres:{}@{}:{}/{}", option.),
        )
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("failed to spawn");

    let stdout = child.stdout.take().expect("Could not take stdout");
    let mut reader = BufReader::new(stdout).lines();
    while let Some(line) = reader.next_line().await.unwrap() {
        if line.contains("Started server") {
            break;
        }
    }

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
