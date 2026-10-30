//! Rust-only fake SolidCraft 98 pipe peer, never shipped.
use std::io::{self, BufRead, Write};
fn main() {
    assert!(std::env::args().any(|a| a == "--remote"));
    let mode = std::env::current_exe()
        .unwrap()
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let mut err = io::stderr();
    for _ in 0..512 {
        err.write_all(&[b'e'; 4096]).unwrap();
    }
    if mode.contains("blocked") {
        std::thread::sleep(std::time::Duration::from_secs(40));
        return;
    }
    let mut offered = true;
    let mut looks = 0;
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        assert_ne!(line, "quit");
        if mode.contains("eof") {
            print!("incomplete\n");
            io::stdout().flush().unwrap();
            return;
        }
        if mode.contains("hang") {
            std::thread::sleep(std::time::Duration::from_secs(40));
            return;
        }
        if line != "look" && !line.starts_with("how") {
            offered = line.starts_with("menu");
        }
        if line == "look" {
            looks += 1;
        }
        let screen = format!("SolidCraft 98, {} x 600.\r\nMenu bar: File, Sketch, Features\r\nToolbar buttons: New Part\r\nTutor: {}\r\n.\r\n", if mode.contains("stale") && looks>1 {100}else{800},if offered {"No part is open. Say \"yes\" to do: menu File > New"}else{"No known next step."});
        let answer = if line == "look" {
            screen.as_str()
        } else if line.starts_with("menu") {
            "error: missing item. Did you mean New? Say \"yes\" to do: menu File > New\r\n.\r\n"
        } else if line.starts_with("how") {
            "Help from application.\n..dot\n.\n"
        }
        // The Windows build knows only the short verbs.
        else if line.starts_with("doubleclick ") || line.starts_with("rightclick ") {
            "error: not a command\r\n.\r\n"
        } else {
            "ok\n.\n"
        };
        for byte in answer.bytes() {
            io::stdout().write_all(&[byte]).unwrap();
        }
        io::stdout().flush().unwrap();
    }
}
