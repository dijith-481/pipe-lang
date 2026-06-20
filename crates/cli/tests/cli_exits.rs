use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_source_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("pipe-lang-{name}-{nonce}.pp"))
}

#[test]
fn test_cli_exits() {
    let bin = env!("CARGO_BIN_EXE_pipe-lang");
    let good = temp_source_path("good");
    let bad = temp_source_path("bad");

    fs::write(&good, "let x = 42").expect("write good source");
    fs::write(&bad, "let =").expect("write bad source");

    let good_status = Command::new(bin)
        .arg("check")
        .arg(&good)
        .status()
        .expect("run pipe-lang check for good source");
    let bad_status = Command::new(bin)
        .arg("check")
        .arg(&bad)
        .status()
        .expect("run pipe-lang check for bad source");

    let _ = fs::remove_file(&good);
    let _ = fs::remove_file(&bad);

    assert!(good_status.success());
    assert_eq!(bad_status.code(), Some(1));
}
