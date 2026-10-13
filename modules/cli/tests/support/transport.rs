#![allow(dead_code)] // Integration targets exercise different transport subsets.
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};
pub fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .args(args)
        .output()
        .expect("CLI")
}
pub fn request(method: &str, params: Value, mcp: bool) -> Value {
    let mut process = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .arg(if mcp { "mcp" } else { "agent" })
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("process");
    let message = if mcp {
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":method,"arguments":params}})
    } else {
        json!({"jsonrpc":"2.0","id":1,"method":method,"params":params})
    };
    writeln!(process.stdin.take().expect("stdin"), "{message}").expect("write");
    let output = process.wait_with_output().expect("response");
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).expect("JSON")
}
pub fn logical_files(path: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(path).expect("dir") {
        let entry = entry.expect("entry");
        if entry.path().is_file() {
            files.push((
                entry.file_name().to_string_lossy().into_owned(),
                std::fs::read(entry.path()).expect("file"),
            ));
        } else {
            for (name, bytes) in logical_files(&entry.path()) {
                files.push((
                    format!("{}/{name}", entry.file_name().to_string_lossy()),
                    bytes,
                ));
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}
