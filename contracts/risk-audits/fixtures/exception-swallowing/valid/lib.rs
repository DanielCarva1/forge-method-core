// exception-swallowing fixture (valid): clean Rust source.
//
// This file passes every rule in exception-swallowing.yaml. It reads a
// config file and handles every error path explicitly.

use std::fs;

fn read_config(path: &str) -> Result<String, std::io::Error> {
    fs::read_to_string(path)
}

fn main() {
    match read_config("config.toml") {
        Ok(content) => println!("loaded: {content}"),
        Err(err) => eprintln!("failed: {err}"),
    }
}
